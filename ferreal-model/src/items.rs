//! The Rust items Ferreal exposes to Unreal, parsed from their syntax and attributes.

use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{
    FnArg, ImplItem, ItemEnum, ItemFn, ItemImpl, ItemStruct, ItemTrait, Pat, ReturnType, TraitItem,
    Type,
};

use crate::attr::{Arg, ArgValue, DefaultValue, args_of, docs, has};
use crate::names::{pascal, unraw};
use crate::ty::{Ty, Use};

/// A `#[uclass]` struct: a `UCLASS` whose `UPROPERTY`s are its `#[uproperty]` fields and
/// whose Rust state is its other fields.
#[derive(Clone, Debug)]
pub struct Class {
    pub name: String,
    pub cpp_name: String,
    /// The C++ class it derives from (`UWidget`, a C++ base of the plugin...).
    pub parent: String,
    /// Headers the generated header includes (the parent's).
    pub includes: Vec<String>,
    /// `UCLASS(...)` specifiers, rendered.
    pub specifiers: Vec<String>,
    /// The `#[cpp_base]` traits whose C++ methods it can call (Rust paths).
    pub bases: Vec<syn::Path>,
    pub properties: Vec<Property>,
    /// Its Rust-only fields.
    pub fields: Vec<syn::Field>,
    pub doc: Vec<String>,
}

/// A `#[uproperty]` field: stored on the Unreal object, read and written from Rust.
#[derive(Clone, Debug)]
pub struct Property {
    pub name: String,
    pub cpp_name: String,
    pub ty: Ty,
    pub rust_ty: Type,
    /// `UPROPERTY(...)` specifiers, rendered.
    pub specifiers: Vec<String>,
    pub default: Option<DefaultValue>,
    /// Generates a `BlueprintSetter` (`Set<Name>`) that notifies the class' `#[on_changed]`.
    pub setter: bool,
    pub doc: Vec<String>,
}

/// How a method is called.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Receiver {
    None,
    Ref,
    Mut,
}

/// When an override calls the C++ parent's version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallSuper {
    Before,
    After,
    Never,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MethodKind {
    /// `#[ufunction(...)]`: a `UFUNCTION` with these specifiers.
    Function { specifiers: Vec<String> },
    /// `#[uoverride]`: overrides a virtual function of the C++ parent.
    Override {
        call_super: CallSuper,
        is_const: bool,
    },
    /// `#[on_changed]`: called when a property changes (setters, the editor).
    OnChanged,
    /// `#[init]`: called once the Rust state of a new object exists.
    Init,
}

#[derive(Clone, Debug)]
pub struct Param {
    pub name: String,
    pub cpp_name: String,
    pub ty: Ty,
}

/// A method of a `#[uclass_impl]` block that C++ calls.
#[derive(Clone, Debug)]
pub struct Method {
    pub name: String,
    pub cpp_name: String,
    pub kind: MethodKind,
    pub receiver: Receiver,
    pub params: Vec<Param>,
    pub ret: Ty,
    pub doc: Vec<String>,
}

/// A `#[uclass_impl]` block.
#[derive(Clone, Debug)]
pub struct ClassImpl {
    pub class: String,
    pub methods: Vec<Method>,
}

/// A `#[uenum]`: a `UENUM` (`enum class E...: uint8`).
#[derive(Clone, Debug)]
pub struct Enum {
    pub name: String,
    pub cpp_name: String,
    pub specifiers: Vec<String>,
    pub variants: Vec<Variant>,
    pub doc: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Variant {
    pub name: String,
    pub value: u8,
    /// `UMETA(...)` contents.
    pub meta: Option<String>,
    pub doc: Vec<String>,
}

/// A `#[cpp_base]` trait: methods of a C++ class that Rust calls.
#[derive(Clone, Debug)]
pub struct Base {
    pub name: String,
    pub cpp_class: String,
    pub includes: Vec<String>,
    pub methods: Vec<BaseMethod>,
}

#[derive(Clone, Debug)]
pub struct BaseMethod {
    pub name: String,
    pub cpp_name: String,
    pub params: Vec<Param>,
    pub ret: Ty,
}

/// An `engine_class!`: a C++ class Rust refers to (`Obj<C>`), with the `#[cpp_base]` traits
/// it implements.
#[derive(Clone, Debug)]
pub struct EngineClass {
    pub vis: syn::Visibility,
    pub name: syn::Ident,
    pub cpp_class: String,
    pub include: Option<String>,
    pub bases: Vec<syn::Path>,
}

/// A `ferreal::engine!(AActor, UGameplayStatics)`: the engine classes (C++ names) a crate
/// calls, whose bindings `ferreal-build` generates from Unreal's reflection data.
#[derive(Clone, Debug)]
pub struct EngineList {
    pub classes: Vec<syn::Ident>,
}

impl Parse for EngineList {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let classes = input.parse_terminated(syn::Ident::parse, syn::Token![,])?;
        if classes.is_empty() {
            return Err(input.error("expected engine classes: `ferreal::engine!(AActor, ...)`"));
        }
        for c in &classes {
            let s = c.to_string();
            let prefixed =
                s.starts_with(['U', 'A']) && s[1..].starts_with(|c: char| c.is_ascii_uppercase());
            if !prefixed {
                return Err(syn::Error::new(
                    c.span(),
                    "engine classes are named as in C++, with their prefix (`AActor`, `UObject`)",
                ));
            }
        }
        Ok(EngineList {
            classes: classes.into_iter().collect(),
        })
    }
}

/// An `#[export]` function: a free Rust function C++ calls (`Ferreal::<Module>::<Name>`).
#[derive(Clone, Debug)]
pub struct Export {
    pub name: String,
    pub cpp_name: String,
    pub params: Vec<Param>,
    pub ret: Ty,
    pub doc: Vec<String>,
}

fn unexpected(a: &Arg, what: &str) -> syn::Error {
    a.error(format!("`{}` is not an argument of {what}", a.name))
}

/// Renders the arguments shared by Unreal's specifier lists: flags as they are,
/// `category = "..."`, `meta(...)`.
fn common_specifier(a: &Arg) -> syn::Result<Option<String>> {
    Ok(match (&a.value, a.name.as_str()) {
        (ArgValue::Flag, _) => Some(a.name.clone()),
        (_, "category") => Some(format!("Category=\"{}\"", a.string()?)),
        (_, "meta") => Some(a.unreal_meta()?),
        (_, "raw") => Some(a.string()?),
        _ => None,
    })
}

fn last_ident(path: &syn::Path) -> String {
    path.segments
        .last()
        .map(|s| s.ident.to_string())
        .unwrap_or_default()
}

impl Class {
    /// `args` are the `#[uclass(...)]` arguments of `item`.
    pub fn parse(args: &[Arg], item: &ItemStruct) -> syn::Result<Class> {
        let name = item.ident.to_string();
        let mut parent = None;
        let mut cpp_name = None;
        let mut includes = Vec::new();
        let mut specifiers = Vec::new();
        let mut bases = Vec::new();
        for a in args {
            match a.name.as_str() {
                "parent" => parent = Some(a.string()?),
                "cpp_name" => cpp_name = Some(a.string()?),
                "include" => includes.push(a.string()?),
                "class_group" => specifiers.push(format!("ClassGroup=({})", a.string()?)),
                "config" => specifiers.push(format!("Config={}", a.string()?)),
                "bases" => {
                    let ArgValue::List(list) = &a.value else {
                        return Err(a.error("expected `bases(Trait, ...)`"));
                    };
                    bases.extend(list.iter().map(|b| b.path.clone()));
                }
                _ => {
                    specifiers.push(common_specifier(a)?.ok_or_else(|| unexpected(a, "#[uclass]"))?)
                }
            }
        }
        let parent = parent.ok_or_else(|| {
            syn::Error::new(
                item.ident.span(),
                "#[uclass] needs `parent = \"UCppClass\"`",
            )
        })?;
        let prefix = if parent.starts_with('A') { "A" } else { "U" };
        let cpp_name = cpp_name.unwrap_or_else(|| format!("{prefix}{name}"));
        let syn::Fields::Named(fields) = &item.fields else {
            return Err(syn::Error::new(
                item.ident.span(),
                "#[uclass] structs have named fields",
            ));
        };
        let mut properties = Vec::new();
        let mut rust_fields = Vec::new();
        for f in &fields.named {
            match args_of(&f.attrs, "uproperty")? {
                Some(args) => properties.push(Property::parse(&args, f)?),
                None => rust_fields.push(f.clone()),
            }
        }
        if !item.generics.params.is_empty() {
            return Err(syn::Error::new(
                item.generics.span(),
                "#[uclass] structs are not generic",
            ));
        }
        Ok(Class {
            name,
            cpp_name,
            parent,
            includes,
            specifiers,
            bases,
            properties,
            fields: rust_fields,
            doc: docs(&item.attrs),
        })
    }
}

impl Property {
    fn parse(args: &[Arg], field: &syn::Field) -> syn::Result<Property> {
        let ident = field.ident.as_ref().expect("named field");
        let name = ident.to_string();
        let ty = Ty::parse(&field.ty, Use::Property)?;
        let mut cpp_name = None;
        let mut specifiers = Vec::new();
        let mut default = None;
        let mut setter = false;
        for a in args {
            match (a.name.as_str(), &a.value) {
                ("name", _) => cpp_name = Some(a.string()?),
                ("setter", ArgValue::Flag) => setter = true,
                ("default", ArgValue::Value(e)) => default = Some(DefaultValue::parse(e)?),
                _ => specifiers
                    .push(common_specifier(a)?.ok_or_else(|| unexpected(a, "#[uproperty]"))?),
            }
        }
        let cpp_name = cpp_name.unwrap_or_else(|| {
            let p = pascal(unraw(&name));
            if ty == Ty::Bool { format!("b{p}") } else { p }
        });
        let mut p = Property {
            name,
            cpp_name,
            ty,
            rust_ty: field.ty.clone(),
            specifiers,
            default,
            setter,
            doc: docs(&field.attrs),
        };
        if setter {
            p.specifiers
                .push(format!("BlueprintSetter={}", p.setter_name()));
        }
        Ok(p)
    }

    /// The name of its `BlueprintSetter` (without the `b` of booleans).
    pub fn setter_name(&self) -> String {
        let bare = if self.ty == Ty::Bool {
            self.cpp_name.strip_prefix('b').unwrap_or(&self.cpp_name)
        } else {
            &self.cpp_name
        };
        format!("Set{bare}")
    }
}

/// The receiver and the named, typed parameters of a signature.
fn params(sig: &syn::Signature, usage: Use) -> syn::Result<(Receiver, Vec<Param>)> {
    let mut receiver = Receiver::None;
    let mut out = Vec::new();
    for input in &sig.inputs {
        match input {
            FnArg::Receiver(r) => {
                if r.reference.is_none() {
                    return Err(syn::Error::new_spanned(
                        r,
                        "methods take `&self` or `&mut self`",
                    ));
                }
                receiver = if r.mutability.is_some() {
                    Receiver::Mut
                } else {
                    Receiver::Ref
                };
            }
            FnArg::Typed(t) => {
                let Pat::Ident(id) = &*t.pat else {
                    return Err(syn::Error::new_spanned(
                        &t.pat,
                        "parameters are plain names",
                    ));
                };
                let name = id.ident.to_string();
                out.push(Param {
                    cpp_name: pascal(unraw(&name)),
                    name,
                    ty: Ty::parse(&t.ty, usage)?,
                });
            }
        }
    }
    Ok((receiver, out))
}

fn ret(sig: &syn::Signature) -> syn::Result<Ty> {
    match &sig.output {
        ReturnType::Default => Ok(Ty::Unit),
        ReturnType::Type(_, t) => Ty::parse(t, Use::Return),
    }
}

impl Method {
    /// The method's Ferreal attribute, if it has one.
    pub fn parse(item: &syn::ImplItemFn) -> syn::Result<Option<Method>> {
        let attrs = &item.attrs;
        let name = item.sig.ident.to_string();
        let mut cpp_name = pascal(unraw(&name));
        let kind = if let Some(args) = args_of(attrs, "ufunction")? {
            let mut specifiers = Vec::new();
            for a in &args {
                if a.name == "name" {
                    cpp_name = a.string()?;
                } else {
                    specifiers
                        .push(common_specifier(a)?.ok_or_else(|| unexpected(a, "#[ufunction]"))?);
                }
            }
            MethodKind::Function { specifiers }
        } else if let Some(args) = args_of(attrs, "uoverride")? {
            let mut call_super = None;
            let mut is_const = false;
            for a in &args {
                match (a.name.as_str(), &a.value) {
                    ("name", _) => cpp_name = a.string()?,
                    ("const", ArgValue::Flag) => is_const = true,
                    ("call_super", _) => {
                        call_super = Some(match a.string()?.as_str() {
                            "before" => CallSuper::Before,
                            "after" => CallSuper::After,
                            "never" => CallSuper::Never,
                            _ => {
                                return Err(
                                    a.error("call_super is \"before\", \"after\" or \"never\"")
                                );
                            }
                        });
                    }
                    _ => return Err(unexpected(a, "#[uoverride]")),
                }
            }
            let returns = !matches!(item.sig.output, ReturnType::Default);
            MethodKind::Override {
                call_super: call_super.unwrap_or(if returns {
                    CallSuper::Never
                } else {
                    CallSuper::Before
                }),
                is_const,
            }
        } else if has(attrs, "on_changed") {
            MethodKind::OnChanged
        } else if has(attrs, "init") {
            MethodKind::Init
        } else {
            return Ok(None);
        };
        let (receiver, params) = params(&item.sig, Use::Param)?;
        let ret = ret(&item.sig)?;
        let span = item.sig.ident.span();
        match &kind {
            MethodKind::OnChanged => {
                if receiver != Receiver::Mut || params.len() != 1 || ret != Ty::Unit {
                    return Err(syn::Error::new(
                        span,
                        "#[on_changed] is `fn(&mut self, property: <Class>Property)`",
                    ));
                }
            }
            MethodKind::Init => {
                if receiver != Receiver::Mut || !params.is_empty() || ret != Ty::Unit {
                    return Err(syn::Error::new(span, "#[init] is `fn(&mut self)`"));
                }
            }
            MethodKind::Override { .. } if receiver == Receiver::None => {
                return Err(syn::Error::new(
                    span,
                    "#[uoverride] methods take `&self` or `&mut self`",
                ));
            }
            _ => {}
        }
        if item.sig.asyncness.is_some() || !item.sig.generics.params.is_empty() {
            return Err(syn::Error::new(
                span,
                "Ferreal methods are neither async nor generic",
            ));
        }
        let params = if matches!(kind, MethodKind::OnChanged) {
            Vec::new()
        } else {
            params
        };
        Ok(Some(Method {
            name,
            cpp_name,
            kind,
            receiver,
            params,
            ret,
            doc: docs(attrs),
        }))
    }
}

impl ClassImpl {
    pub fn parse(item: &ItemImpl) -> syn::Result<ClassImpl> {
        if item.trait_.is_some() {
            return Err(syn::Error::new_spanned(
                &item.self_ty,
                "#[uclass_impl] is on an inherent impl",
            ));
        }
        let class = match &*item.self_ty {
            Type::Path(p) => last_ident(&p.path),
            t => return Err(syn::Error::new_spanned(t, "expected a #[uclass] struct")),
        };
        let mut methods = Vec::new();
        for it in &item.items {
            if let ImplItem::Fn(f) = it
                && let Some(m) = Method::parse(f)?
            {
                methods.push(m);
            }
        }
        let count = |k: fn(&MethodKind) -> bool| methods.iter().filter(|m| k(&m.kind)).count();
        if count(|k| matches!(k, MethodKind::OnChanged)) > 1
            || count(|k| matches!(k, MethodKind::Init)) > 1
        {
            return Err(syn::Error::new_spanned(
                &item.self_ty,
                "one #[on_changed] and one #[init] at most",
            ));
        }
        Ok(ClassImpl { class, methods })
    }
}

impl Enum {
    pub fn parse(args: &[Arg], item: &ItemEnum) -> syn::Result<Enum> {
        let name = item.ident.to_string();
        let mut cpp_name = None;
        let mut specifiers = Vec::new();
        for a in args {
            if a.name == "cpp_name" {
                cpp_name = Some(a.string()?);
            } else {
                specifiers.push(common_specifier(a)?.ok_or_else(|| unexpected(a, "#[uenum]"))?);
            }
        }
        if specifiers.is_empty() {
            specifiers.push("BlueprintType".into());
        }
        let mut variants = Vec::new();
        let mut next: u16 = 0;
        for v in &item.variants {
            if !matches!(v.fields, syn::Fields::Unit) {
                return Err(syn::Error::new_spanned(
                    v,
                    "#[uenum] variants carry no data",
                ));
            }
            if let Some((_, e)) = &v.discriminant {
                let text = crate::attr::literal_text(e).and_then(|t| t.parse::<u16>().ok());
                next = text.ok_or_else(|| syn::Error::new_spanned(e, "expected a number"))?;
            }
            let value = u8::try_from(next)
                .map_err(|_| syn::Error::new_spanned(v, "#[uenum] values fit in a u8"))?;
            let meta = match args_of(&v.attrs, "umeta")? {
                Some(args) => Some(
                    args.iter()
                        .map(|a| match &a.value {
                            ArgValue::Flag => Ok(a.name.clone()),
                            ArgValue::Value(e) => Ok(format!(
                                "{}=\"{}\"",
                                a.name,
                                crate::attr::literal_text(e)
                                    .ok_or_else(|| a.error("expected a literal"))?
                            )),
                            ArgValue::List(_) => Err(a.error("expected `Key = value`")),
                        })
                        .collect::<syn::Result<Vec<_>>>()?
                        .join(", "),
                ),
                None => None,
            };
            variants.push(Variant {
                name: v.ident.to_string(),
                value,
                meta,
                doc: docs(&v.attrs),
            });
            next += 1;
        }
        Ok(Enum {
            cpp_name: cpp_name.unwrap_or_else(|| format!("E{name}")),
            name,
            specifiers,
            variants,
            doc: docs(&item.attrs),
        })
    }
}

impl Base {
    pub fn parse(args: &[Arg], item: &ItemTrait) -> syn::Result<Base> {
        let mut cpp_class = None;
        let mut includes = Vec::new();
        for a in args {
            match a.name.as_str() {
                "class" => cpp_class = Some(a.string()?),
                "include" => includes.push(a.string()?),
                _ => return Err(unexpected(a, "#[cpp_base]")),
            }
        }
        let cpp_class = cpp_class.ok_or_else(|| {
            syn::Error::new(
                item.ident.span(),
                "#[cpp_base] needs `class = \"UCppClass\"`",
            )
        })?;
        let mut methods = Vec::new();
        for it in &item.items {
            let TraitItem::Fn(f) = it else {
                return Err(syn::Error::new_spanned(
                    it,
                    "#[cpp_base] traits hold methods only",
                ));
            };
            if f.default.is_some() {
                return Err(syn::Error::new_spanned(
                    &f.sig,
                    "#[cpp_base] methods have no body: C++ implements them",
                ));
            }
            let name = f.sig.ident.to_string();
            let mut cpp_name = pascal(unraw(&name));
            if let Some(args) = args_of(&f.attrs, "cpp")? {
                for a in &args {
                    match a.name.as_str() {
                        "name" => cpp_name = a.string()?,
                        _ => return Err(unexpected(a, "#[cpp]")),
                    }
                }
            }
            let (receiver, params) = params(&f.sig, Use::Param)?;
            if receiver != Receiver::Ref {
                return Err(syn::Error::new_spanned(
                    &f.sig,
                    "#[cpp_base] methods take `&self`",
                ));
            }
            methods.push(BaseMethod {
                name,
                cpp_name,
                params,
                ret: ret(&f.sig)?,
            });
        }
        if !item.generics.params.is_empty() || !item.supertraits.is_empty() {
            return Err(syn::Error::new(
                item.ident.span(),
                "#[cpp_base] traits have no generics or supertraits",
            ));
        }
        Ok(Base {
            name: item.ident.to_string(),
            cpp_class,
            includes,
            methods,
        })
    }
}

/// `engine_class!(pub FontFace = "UFontFace" in "Engine/FontFace.h", bases(Trait, ...));`
impl Parse for EngineClass {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let vis: syn::Visibility = input.parse()?;
        let name: syn::Ident = input.parse()?;
        input.parse::<syn::Token![=]>()?;
        let cpp_class: syn::LitStr = input.parse()?;
        let include = if input.peek(syn::Token![in]) {
            input.parse::<syn::Token![in]>()?;
            Some(input.parse::<syn::LitStr>()?.value())
        } else {
            None
        };
        let mut bases = Vec::new();
        if input.peek(syn::Token![,]) {
            input.parse::<syn::Token![,]>()?;
            let kw: syn::Ident = input.parse()?;
            if kw != "bases" {
                return Err(syn::Error::new(kw.span(), "expected `bases(...)`"));
            }
            let content;
            syn::parenthesized!(content in input);
            bases = content
                .parse_terminated(syn::Path::parse, syn::Token![,])?
                .into_iter()
                .collect();
        }
        input.parse::<Option<syn::Token![;]>>()?;
        Ok(EngineClass {
            vis,
            name,
            cpp_class: cpp_class.value(),
            include,
            bases,
        })
    }
}

impl Export {
    pub fn parse(args: &[Arg], item: &ItemFn) -> syn::Result<Export> {
        let name = item.sig.ident.to_string();
        let mut cpp_name = pascal(unraw(&name));
        for a in args {
            match a.name.as_str() {
                "name" => cpp_name = a.string()?,
                _ => return Err(unexpected(a, "#[export]")),
            }
        }
        let (receiver, params) = params(&item.sig, Use::Param)?;
        if receiver != Receiver::None {
            return Err(syn::Error::new_spanned(
                &item.sig,
                "#[export] is on free functions",
            ));
        }
        if item.sig.asyncness.is_some() || !item.sig.generics.params.is_empty() {
            return Err(syn::Error::new_spanned(
                &item.sig,
                "#[export] functions are neither async nor generic",
            ));
        }
        Ok(Export {
            name,
            cpp_name,
            params,
            ret: ret(&item.sig)?,
            doc: docs(&item.attrs),
        })
    }
}

/// The marker of a `#[cpp_base]` trait at `path`: the trait object type `dyn path`, which
/// classes implement `Derives<_>` of (nameable wherever the trait is).
pub fn base_marker(path: &syn::Path) -> Type {
    syn::parse_quote!(dyn #path)
}

/// The name a base path refers to.
pub fn path_name(path: &syn::Path) -> String {
    last_ident(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attr::parse_args;
    use quote::quote;

    #[test]
    fn class_and_impl() {
        let item: ItemStruct = syn::parse2(quote! {
            /// A label.
            pub struct Label {
                #[uproperty(EditAnywhere, BlueprintReadWrite, category = "Text", default = "Hi", setter)]
                text: String,
                #[uproperty(EditAnywhere, default = true)]
                autowrap: bool,
                #[uproperty(EditAnywhere)]
                font: Option<Obj<FontFace>>,
                cache: Vec<u8>,
            }
        })
        .unwrap();
        let args = parse_args(quote!(
            parent = "UWidget",
            include = "Components/Widget.h",
            BlueprintType,
            bases(crate::Base)
        ))
        .unwrap();
        let c = Class::parse(&args, &item).unwrap();
        assert_eq!(c.cpp_name, "ULabel");
        assert_eq!(c.specifiers, ["BlueprintType"]);
        assert_eq!(c.properties.len(), 3);
        assert_eq!(c.fields.len(), 1);
        assert_eq!(
            c.properties[0].specifiers.last().unwrap(),
            "BlueprintSetter=SetText"
        );
        assert_eq!(c.properties[1].cpp_name, "bAutowrap");
        assert_eq!(c.doc, ["A label."]);
        assert_eq!(path_name(&c.bases[0]), "Base");
        let marker = base_marker(&c.bases[0]);
        assert_eq!(
            quote!(#marker).to_string(),
            quote!(dyn crate::Base).to_string()
        );

        let imp: ItemImpl = syn::parse2(quote! {
            impl Label {
                #[ufunction(BlueprintCallable, category = "Text")]
                fn text_size(&mut self) -> Vector2 { todo!() }
                #[uoverride(const)]
                fn compute_desired_size(&self, scale: f32) -> Vector2 { todo!() }
                #[uoverride]
                fn synchronize_properties(&mut self) {}
                #[on_changed]
                fn changed(&mut self, p: LabelProperty) {}
                fn helper(&self) {}
            }
        })
        .unwrap();
        let i = ClassImpl::parse(&imp).unwrap();
        assert_eq!(i.methods.len(), 4);
        assert_eq!(i.methods[0].cpp_name, "TextSize");
        assert_eq!(
            i.methods[1].kind,
            MethodKind::Override {
                call_super: CallSuper::Never,
                is_const: true
            }
        );
        assert_eq!(
            i.methods[2].kind,
            MethodKind::Override {
                call_super: CallSuper::Before,
                is_const: false
            }
        );
        assert!(i.methods[3].params.is_empty());
    }

    #[test]
    fn enums_bases_engine_classes() {
        let e: ItemEnum = syn::parse2(
            quote! { enum Align { Start, #[umeta(DisplayName = "Middle")] Center = 4, End } },
        )
        .unwrap();
        let e = Enum::parse(&[], &e).unwrap();
        assert_eq!(e.cpp_name, "EAlign");
        assert_eq!(
            e.variants.iter().map(|v| v.value).collect::<Vec<_>>(),
            [0, 4, 5]
        );
        assert_eq!(
            e.variants[1].meta.as_deref(),
            Some("DisplayName=\"Middle\"")
        );

        let t: ItemTrait =
            syn::parse2(quote! { trait Draws { fn set_quads(&self, quads: &[f32], name: &str); } })
                .unwrap();
        let args = parse_args(quote!(class = "UDrawBase", include = "DrawBase.h")).unwrap();
        let b = Base::parse(&args, &t).unwrap();
        assert_eq!(b.methods[0].cpp_name, "SetQuads");

        let ec: EngineClass =
            syn::parse2(quote!(pub FontFace = "UFontFace" in "Engine/FontFace.h", bases(Draws)))
                .unwrap();
        assert_eq!(ec.cpp_class, "UFontFace");
        assert_eq!(ec.bases.len(), 1);

        let list: EngineList = syn::parse2(quote!(AActor, UGameplayStatics,)).unwrap();
        assert_eq!(list.classes.len(), 2);
        assert!(syn::parse2::<EngineList>(quote!(Actor)).is_err());
        assert!(syn::parse2::<EngineList>(quote!()).is_err());
    }
}
