//! Rivet's attribute macros. They write the Rust half of the glue; `rivet-build` writes the
//! C++ half from the same items (both parse them with `rivet-model`). Use them through the
//! `rivet` crate, whose documentation describes them.

mod convert;

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as Tokens};
use quote::{format_ident, quote};
use rivet_model::{
    Base, Class, ClassImpl, EngineClass, Enum, Export, MethodKind, Receiver, Symbols,
};
use syn::{ImplItem, ItemEnum, ItemFn, ItemImpl, ItemStruct, ItemTrait, TraitItem};

use crate::convert::{arg_out, ffi_type, param_in, return_out, setter_type, value_in};

/// The Unreal module the crate belongs to, which `rivet-build` passes in [`rivet_model::MODULE_ENV`].
fn module_symbols() -> syn::Result<Symbols> {
    match std::env::var(rivet_model::MODULE_ENV) {
        Ok(m) if !m.is_empty() => Ok(Symbols::new(&m)),
        _ => Err(syn::Error::new(
            Span::call_site(),
            format!(
                "rivet: {} is not set; call rivet_build::Module::new(\"<UnrealModule>\")...run() from build.rs",
                rivet_model::MODULE_ENV
            ),
        )),
    }
}

fn expand(r: syn::Result<Tokens>) -> TokenStream {
    r.unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Removes the attributes Rivet reads (`#[uproperty]`, `#[ufunction]`...) from `attrs`.
fn strip(attrs: &mut Vec<syn::Attribute>, names: &[&str]) {
    attrs.retain(|a| {
        !a.path()
            .segments
            .last()
            .is_some_and(|s| names.iter().any(|n| s.ident == n))
    });
}

fn sym(name: &str) -> syn::Ident {
    syn::Ident::new(name, Span::call_site())
}

/// `#[uclass(parent = "UCppClass", include = "Header.h", ...)]` on a struct: a `UCLASS`.
///
/// `#[uproperty(...)]` fields are stored on the Unreal object and read with generated
/// methods (`self.text()`, `self.set_text(...)`); the other fields are the Rust state of each
/// object, created with `Default::default()`. Arguments: `parent` (required), `include`
/// (repeatable), `cpp_name`, `bases(Trait, ...)` (`#[cpp_base]` traits of the parent), and
/// `UCLASS` specifiers (`BlueprintType`, `meta(...)`, `class_group = "..."`,
/// `config = "..."`, `raw = "..."`).
#[proc_macro_attribute]
pub fn uclass(args: TokenStream, item: TokenStream) -> TokenStream {
    expand(uclass_impl_inner(args.into(), item.into()))
}

fn uclass_impl_inner(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    let symbols = module_symbols()?;
    let args = rivet_model::parse_args(args)?;
    let mut item: ItemStruct = syn::parse2(item)?;
    let class = Class::parse(&args, &item)?;
    let ident = item.ident.clone();
    let vis = item.vis.clone();
    let name = class.name.as_str();
    let cpp = class.cpp_name.as_str();
    let table = format_ident!("RivetTable_{}", ident);
    let table_static = format_ident!("RIVET_TABLE_{}", ident);
    let property_enum = format_ident!("{}Property", ident);
    let init_trait = format_ident!("RivetInit_{}", ident);

    let mut table_fields = Vec::new();
    let mut accessors = Vec::new();
    let mut variants = Vec::new();
    for p in &class.properties {
        let field = format_ident!("{}", p.name);
        let get = format_ident!("get_{}", p.name.trim_start_matches("r#"));
        let set = format_ident!("set_{}", p.name.trim_start_matches("r#"));
        let fin = ffi_type(&p.ty, true);
        let fout = ffi_type(&p.ty, false);
        table_fields.push(quote! {
            pub #get: extern "C" fn(::rivet::ffi::RawObj) -> #fin,
            pub #set: extern "C" fn(::rivet::ffi::RawObj, #fout),
        });
        let rust_ty = &p.rust_ty;
        let what = format!("{name}.{}", p.name);
        let read = value_in(&p.ty, &quote!(v), &what);
        let setter_ty = setter_type(&p.ty, rust_ty);
        let value = format_ident!("value");
        let (keep, carrier) = arg_out(&p.ty, &value);
        let doc = format!("`{}`: reads the `UPROPERTY` from the object.", p.cpp_name);
        let set_doc = format!(
            "Writes `{}` on the object (Unreal is not notified: no `#[on_changed]`).",
            p.cpp_name
        );
        accessors.push(quote! {
            #[doc = #doc]
            #[allow(clippy::let_and_return)]
            pub fn #field(&self) -> #rust_ty {
                let v = (#table_static.get().#get)(self.rivet_object);
                #read
            }
            #[doc = #set_doc]
            pub fn #set(&self, #value: #setter_ty) {
                #keep
                (#table_static.get().#set)(self.rivet_object, #carrier);
            }
        });
        let variant = format_ident!("{}", rivet_model::pascal(p.name.trim_start_matches("r#")));
        variants.push(variant);
    }

    let defaults = class.fields.iter().map(|f| {
        let id = f.ident.as_ref().expect("named");
        quote!(#id: ::core::default::Default::default())
    });
    let bases = class.bases.iter().map(|b| {
        let marker = rivet_model::base_marker(b);
        quote!(impl ::rivet::Derives<#marker> for #ident {})
    });

    // Keep the Rust fields; the object handle replaces the properties.
    let syn::Fields::Named(fields) = &mut item.fields else {
        unreachable!("checked by Class::parse")
    };
    let kept: Vec<syn::Field> = fields
        .named
        .iter()
        .filter(|f| {
            rivet_model::attr::args_of(&f.attrs, "uproperty")
                .ok()
                .flatten()
                .is_none()
        })
        .cloned()
        .collect();
    fields.named = kept.into_iter().collect();
    fields.named.insert(
        0,
        syn::parse_quote! {
            #[doc(hidden)]
            rivet_object: ::rivet::ffi::RawObj
        },
    );
    strip(&mut item.attrs, &["uclass"]);

    let new = sym(&symbols.new_state(name));
    let drop = sym(&symbols.drop_state(name));
    let register = sym(&symbols.register_class(name));
    let table_doc = format!("The C++ accessors of `{cpp}`'s properties (generated).");
    let enum_doc = format!("The `UPROPERTY`s of `{cpp}`, as `#[on_changed]` receives them.");

    Ok(quote! {
        #item

        #[doc = #table_doc]
        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        #[repr(C)]
        #vis struct #table {
            #(#table_fields)*
        }

        #[doc(hidden)]
        #[allow(non_upper_case_globals)]
        #vis static #table_static: ::rivet::ffi::Table<#table> = ::rivet::ffi::Table::new(#name);

        #[doc = #enum_doc]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #vis enum #property_enum {
            #(#variants,)*
        }

        impl #property_enum {
            /// The property at `index` in declaration order.
            pub fn from_index(index: u32) -> ::core::option::Option<Self> {
                const ALL: &[#property_enum] = &[#(#property_enum::#variants,)*];
                ALL.get(index as usize).copied()
            }
        }

        impl #ident {
            #(#accessors)*

            #[doc(hidden)]
            fn rivet_new(object: ::rivet::ffi::RawObj) -> Self {
                #ident {
                    rivet_object: object,
                    #(#defaults,)*
                }
            }
        }

        impl ::rivet::AsObject for #ident {
            fn raw_object(&self) -> ::rivet::ffi::RawObj {
                self.rivet_object
            }
        }

        impl ::rivet::Class for #ident {
            const CPP: &'static str = #cpp;
        }

        #(#bases)*

        /// Called on new objects; an `#[init]` method shadows this default.
        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        trait #init_trait {
            fn rivet_call_init(&mut self) {}
        }
        impl #init_trait for #ident {}

        #[doc(hidden)]
        #[unsafe(no_mangle)]
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        pub extern "C" fn #register(table: *const #table) {
            #table_static.set(table);
        }

        #[doc(hidden)]
        #[unsafe(no_mangle)]
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        pub extern "C" fn #new(object: ::rivet::ffi::RawObj) -> ::rivet::ffi::RawObj {
            let what = concat!(#name, "::new");
            ::rivet::ffi::guard(what, || {
                let mut value = #ident::rivet_new(object);
                #[allow(unused_imports)]
                use #init_trait as _;
                value.rivet_call_init();
                ::rivet::ffi::RawObj(::rivet::ffi::State::into_raw(value, #name))
            })
        }

        #[doc(hidden)]
        #[unsafe(no_mangle)]
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        pub extern "C" fn #drop(state: *mut ::core::ffi::c_void) {
            // SAFETY: C++ passes the state `new` returned, once, when the object is destroyed.
            ::rivet::ffi::guard(concat!(#name, "::drop"), || unsafe { ::rivet::ffi::State::<#ident>::drop_raw(state) });
        }
    })
}

/// `#[uclass_impl]` on the `impl` block of a `#[uclass]` struct: its methods marked
/// `#[ufunction(...)]` (`UFUNCTION`s), `#[uoverride]` (overrides of the C++ parent's virtual
/// functions; `const`, `call_super = "before" | "after" | "never"`, `name = "..."`),
/// `#[on_changed]` (`fn(&mut self, property: <Class>Property)`, called when a property
/// changes from a setter or the editor) and `#[init]` (`fn(&mut self)`, called once a new
/// object's Rust state exists) are called from C++.
#[proc_macro_attribute]
pub fn uclass_impl(_args: TokenStream, item: TokenStream) -> TokenStream {
    expand(uclass_impl_block(item.into()))
}

fn uclass_impl_block(item: Tokens) -> syn::Result<Tokens> {
    let symbols = module_symbols()?;
    let mut item: ItemImpl = syn::parse2(item)?;
    let model = ClassImpl::parse(&item)?;
    let self_ty = item.self_ty.clone();
    let class = model.class.as_str();
    let property_enum = format_ident!("{}Property", class);
    let mut exports = Vec::new();
    let mut init = None;
    for m in &model.methods {
        let method_call: syn::Ident = syn::parse_str(&m.name)?;
        let what = format!("{class}::{}", m.name);
        if m.kind == MethodKind::Init {
            init = Some(quote! {
                impl #self_ty {
                    #[doc(hidden)]
                    #[allow(dead_code)]
                    pub(crate) fn rivet_call_init(&mut self) {
                        self.#method_call();
                    }
                }
            });
            continue;
        }
        let export = sym(&symbols.method(class, &m.name));
        if m.kind == MethodKind::OnChanged {
            exports.push(quote! {
                #[doc(hidden)]
                #[unsafe(no_mangle)]
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
                pub extern "C" fn #export(state: *mut ::core::ffi::c_void, index: u32) {
                    ::rivet::ffi::guard(#what, || {
                        let Some(property) = #property_enum::from_index(index) else { return };
                        // SAFETY: C++ passes the state `new` returned for this object.
                        unsafe { ::rivet::ffi::State::<#self_ty>::with_mut(state, #what, |this| this.#method_call(property)) }
                    })
                }
            });
            continue;
        }
        let mut params = Vec::new();
        let mut prepare = Vec::new();
        let mut args = Vec::new();
        for p in &m.params {
            let name = format_ident!("{}", p.name.trim_start_matches("r#"));
            let ty = ffi_type(&p.ty, true);
            params.push(quote!(#name: #ty));
            let (prep, arg) = param_in(&p.ty, &name, &format!("{what}({})", p.name));
            prepare.push(prep);
            args.push(arg);
        }
        let ret_ty = ffi_type(&m.ret, false);
        let out = return_out(&m.ret, &quote!(r));
        let body = |call: Tokens| quote! { #(#prepare)* let r = #call; #out };
        let func = match m.receiver {
            Receiver::None => {
                let b = body(quote!(<#self_ty>::#method_call(#(#args),*)));
                quote! {
                    pub extern "C" fn #export(#(#params),*) -> #ret_ty {
                        ::rivet::ffi::guard(#what, || { #b })
                    }
                }
            }
            Receiver::Ref | Receiver::Mut => {
                let b = body(quote!(this.#method_call(#(#args),*)));
                let with = if m.receiver == Receiver::Mut {
                    quote!(with_mut)
                } else {
                    quote!(with_ref)
                };
                quote! {
                    pub extern "C" fn #export(state: *mut ::core::ffi::c_void, #(#params),*) -> #ret_ty {
                        ::rivet::ffi::guard(#what, || {
                            // SAFETY: C++ passes the state `new` returned for this object.
                            unsafe { ::rivet::ffi::State::<#self_ty>::#with(state, #what, |this| { #b }) }
                        })
                    }
                }
            }
        };
        exports.push(quote! {
            #[doc(hidden)]
            #[unsafe(no_mangle)]
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
            #[allow(clippy::unit_arg, clippy::let_unit_value)]
            #func
        });
    }
    for it in &mut item.items {
        if let ImplItem::Fn(f) = it {
            strip(
                &mut f.attrs,
                &["ufunction", "uoverride", "on_changed", "init"],
            );
        }
    }
    Ok(quote! {
        #item
        #init
        #(#exports)*
    })
}

/// `#[uenum]` on a fieldless enum: a `UENUM` (`enum class E<Name> : uint8`, `cpp_name` to
/// rename it), `BlueprintType` unless other specifiers are given. Variants take
/// `#[umeta(DisplayName = "...")]`. The macro derives `Clone`, `Copy`, `Debug`,
/// `PartialEq`, `Eq` and `Hash`, and `Default` (the first variant).
#[proc_macro_attribute]
pub fn uenum(args: TokenStream, item: TokenStream) -> TokenStream {
    expand(uenum_inner(args.into(), item.into()))
}

fn uenum_inner(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    let args = rivet_model::parse_args(args)?;
    let mut item: ItemEnum = syn::parse2(item)?;
    let model = Enum::parse(&args, &item)?;
    let ident = item.ident.clone();
    for v in &mut item.variants {
        strip(&mut v.attrs, &["umeta"]);
        // Values are given by the model (explicit or counted on).
        v.discriminant = None;
    }
    let names: Vec<syn::Ident> = model
        .variants
        .iter()
        .map(|v| format_ident!("{}", v.name))
        .collect();
    let values: Vec<u8> = model.variants.iter().map(|v| v.value).collect();
    let first = names
        .first()
        .ok_or_else(|| syn::Error::new_spanned(&ident, "#[uenum] needs a variant"))?;
    for (v, value) in item.variants.iter_mut().zip(&values) {
        v.discriminant = Some((syn::parse_quote!(=), syn::parse_quote!(#value)));
    }
    strip(&mut item.attrs, &["uenum"]);
    Ok(quote! {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[repr(u8)]
        #item

        impl ::core::default::Default for #ident {
            fn default() -> Self {
                #ident::#first
            }
        }

        impl ::rivet::UEnum for #ident {
            fn to_u8(self) -> u8 {
                self as u8
            }
            fn from_u8(v: u8) -> Self {
                match v {
                    #(#values => #ident::#names,)*
                    _ => ::core::default::Default::default(),
                }
            }
        }
    })
}

/// `#[cpp_base(class = "UCppClass", include = "Header.h")]` on a trait: methods of a C++
/// class (written by hand in the plugin) that Rust calls. Each method takes `&self`, has no
/// body, and calls the C++ method of the same name in PascalCase (`#[cpp(name = "...")]` to
/// rename it). Classes listing the trait in `bases(...)` and `engine_class!` handles of
/// classes listing it implement it.
#[proc_macro_attribute]
pub fn cpp_base(args: TokenStream, item: TokenStream) -> TokenStream {
    expand(cpp_base_inner(args.into(), item.into()))
}

fn cpp_base_inner(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    let symbols = module_symbols()?;
    let args = rivet_model::parse_args(args)?;
    let mut item: ItemTrait = syn::parse2(item)?;
    let model = Base::parse(&args, &item)?;
    let ident = item.ident.clone();
    let vis = item.vis.clone();
    let table = format_ident!("RivetBaseTable_{}", ident);
    let table_static = format_ident!("RIVET_BASE_{}", ident);
    let register = sym(&symbols.register_base(&model.name));
    let mut fields = Vec::new();
    let mut impls = Vec::new();
    for (m, it) in model.methods.iter().zip(&item.items) {
        let TraitItem::Fn(f) = it else {
            unreachable!("checked by Base::parse")
        };
        let name = format_ident!("{}", m.name.trim_start_matches("r#"));
        let mut carriers = Vec::new();
        let mut keep = Vec::new();
        let mut args = Vec::new();
        for p in &m.params {
            let pn = format_ident!("{}", p.name.trim_start_matches("r#"));
            carriers.push(ffi_type(&p.ty, false));
            let (k, a) = arg_out(&p.ty, &pn);
            keep.push(k);
            args.push(a);
        }
        let ret = ffi_type(&m.ret, true);
        fields
            .push(quote!(pub #name: extern "C" fn(::rivet::ffi::RawObj #(, #carriers)*) -> #ret,));
        let sig = &f.sig;
        let what = format!("{}::{}", model.name, m.name);
        let read = value_in(&m.ret, &quote!(r), &what);
        impls.push(quote! {
            #[allow(clippy::let_unit_value, clippy::unit_arg)]
            #sig {
                #(#keep)*
                let r = (#table_static.get().#name)(::rivet::AsObject::raw_object(self) #(, #args)*);
                #read
            }
        });
    }
    for it in &mut item.items {
        if let TraitItem::Fn(f) = it {
            strip(&mut f.attrs, &["cpp"]);
        }
    }
    strip(&mut item.attrs, &["cpp_base"]);
    let name = model.name.as_str();
    Ok(quote! {
        #item

        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        #[repr(C)]
        #vis struct #table {
            #(#fields)*
        }

        #[doc(hidden)]
        #[allow(non_upper_case_globals)]
        #vis static #table_static: ::rivet::ffi::Table<#table> = ::rivet::ffi::Table::new(#name);

        #[doc(hidden)]
        #[unsafe(no_mangle)]
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        pub extern "C" fn #register(table: *const #table) {
            #table_static.set(table);
        }

        // Classes deriving from the C++ class say so with `Derives<dyn Trait>`.
        impl<T: ::rivet::AsObject + ::rivet::Derives<dyn #ident>> #ident for T {
            #(#impls)*
        }
    })
}

/// `engine_class!(pub FontFace = "UFontFace" in "Engine/FontFace.h");` declares a marker type
/// for `Obj<FontFace>` handles of a C++ class; `, bases(Trait, ...)` lists the `#[cpp_base]`
/// traits it implements.
#[proc_macro]
pub fn engine_class(item: TokenStream) -> TokenStream {
    expand(engine_class_inner(item.into()))
}

fn engine_class_inner(item: Tokens) -> syn::Result<Tokens> {
    let decl: EngineClass = syn::parse2(item)?;
    let EngineClass {
        vis,
        name,
        cpp_class,
        ..
    } = &decl;
    let doc = format!("Handles of `{cpp_class}` (`Obj<{name}>`).");
    let bases = decl.bases.iter().map(|b| {
        let marker = rivet_model::base_marker(b);
        quote!(impl ::rivet::Derives<#marker> for #name {})
    });
    Ok(quote! {
        #[doc = #doc]
        #vis enum #name {}

        impl ::rivet::Class for #name {
            const CPP: &'static str = #cpp_class;
        }

        #(#bases)*
    })
}

/// `#[export]` on a free function: C++ calls it as `Rivet::<Module>::<Name>(...)`
/// (`name = "..."` to rename it).
#[proc_macro_attribute]
pub fn export(args: TokenStream, item: TokenStream) -> TokenStream {
    expand(export_inner(args.into(), item.into()))
}

fn export_inner(args: Tokens, item: Tokens) -> syn::Result<Tokens> {
    let symbols = module_symbols()?;
    let args = rivet_model::parse_args(args)?;
    let item: ItemFn = syn::parse2(item)?;
    let model = Export::parse(&args, &item)?;
    let ident = &item.sig.ident;
    let export = sym(&symbols.export(&model.name));
    let what = model.name.clone();
    let mut params = Vec::new();
    let mut prepare = Vec::new();
    let mut call = Vec::new();
    for p in &model.params {
        let name = format_ident!("{}", p.name.trim_start_matches("r#"));
        let ty = ffi_type(&p.ty, true);
        params.push(quote!(#name: #ty));
        let (prep, arg) = param_in(&p.ty, &name, &format!("{what}({})", p.name));
        prepare.push(prep);
        call.push(arg);
    }
    let ret_ty = ffi_type(&model.ret, false);
    let out = return_out(&model.ret, &quote!(r));
    Ok(quote! {
        #item

        #[doc(hidden)]
        #[unsafe(no_mangle)]
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        #[allow(clippy::unit_arg, clippy::let_unit_value)]
        pub extern "C" fn #export(#(#params),*) -> #ret_ty {
            ::rivet::ffi::guard(#what, || {
                #(#prepare)*
                let r = #ident(#(#call),*);
                #out
            })
        }
    })
}

/// `rivet::module!();` once in the crate root: the module's entry point, through which C++
/// hands Rust its logging function when the Unreal module starts, and the crate's `engine`
/// module: the bindings `rivet-build` generates for the classes `rivet::engine!` names.
#[proc_macro]
pub fn module(_item: TokenStream) -> TokenStream {
    expand((|| {
        let init = sym(&module_symbols()?.init());
        Ok(quote! {
            #[doc(hidden)]
            #[unsafe(no_mangle)]
            #[allow(clippy::not_unsafe_ptr_arg_deref)]
            pub extern "C" fn #init(table: *const ::rivet::log::ModuleTable) {
                ::rivet::log::init(table);
            }

            /// The engine classes this crate calls (`rivet::engine!`), generated by
            /// `rivet-build` from Unreal's reflection data: a marker type per class, an
            /// `<Class>Api` trait of its reflected functions (implemented by every handle of
            /// it or of a class deriving from it), its static functions on the marker, and the
            /// enums they take.
            #[allow(
                non_camel_case_types,
                non_snake_case,
                dead_code,
                unused_qualifications,
                clippy::all,
                clippy::not_unsafe_ptr_arg_deref
            )]
            pub mod engine {
                include!(concat!(env!("OUT_DIR"), "/rivet_engine.rs"));
            }
        })
    })())
}

/// `rivet::engine!(AActor, UStaticMeshComponent, UGameplayStatics);` names engine classes
/// (by their C++ names) this crate calls. `rivet-build` reads it and generates their
/// bindings into the crate's `engine` module (see `rivet::module!`); the macro itself
/// expands to nothing.
#[proc_macro]
pub fn engine(item: TokenStream) -> TokenStream {
    expand((|| {
        let _: rivet_model::EngineList = syn::parse2(item.into())?;
        Ok(Tokens::new())
    })())
}
