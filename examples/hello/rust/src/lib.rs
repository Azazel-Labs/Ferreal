//! Ferreal's example: `UGreeter`, a `UCLASS` written in Rust, with every kind of property,
//! function and hook Ferreal supports, and calls into engine classes through the generated
//! bindings (`poke`). The C++ it needs is in `../Source/FerrealHello`: the parent class
//! `UHelloBase` (hand-written) and the generated glue (`Ferreal/`).

use ferreal::prelude::*;

ferreal::module!();

// Engine classes this crate calls: ferreal-build generates their bindings (the `engine`
// module) from Unreal's reflection data, kept in ferreal-engine.json.
ferreal::engine!(
    AActor,
    UPrimitiveComponent,
    UKismetStringLibrary,
    UKismetSystemLibrary
);

engine_class!(pub Texture = "UTexture2D" in "Engine/Texture2D.h");
engine_class!(pub Notifier = "UHelloBase" in "HelloBase.h", bases(Notify));

/// Methods of the hand-written C++ parent, `UHelloBase`, that Rust calls.
#[cpp_base(class = "UHelloBase", include = "HelloBase.h")]
pub trait Notify {
    /// Calls `UHelloBase::Notify(const FString&, TConstArrayView<float>)`.
    fn notify(&self, message: &str, values: &[f32]) -> i32;
    /// Calls `UHelloBase::Label() -> FString`.
    fn label(&self) -> String;
    /// Calls `UHelloBase::Bytes() -> TArray<uint8>`.
    fn bytes(&self) -> Vec<u8>;
}

/// How a greeter feels.
#[uenum]
pub enum Mood {
    Calm,
    #[umeta(DisplayName = "Very excited")]
    Excited = 3,
    Sleepy,
}

/// Greets people, from Rust.
#[uclass(
    parent = "UHelloBase",
    include = "HelloBase.h",
    BlueprintType,
    meta(DisplayName = "Greeter"),
    bases(Notify)
)]
pub struct Greeter {
    /// Who to greet.
    #[uproperty(
        EditAnywhere,
        BlueprintReadWrite,
        category = "Greeting",
        default = "World",
        setter
    )]
    name: String,
    #[uproperty(EditAnywhere, category = "Greeting", default = 2.5)]
    scale: f32,
    #[uproperty(EditAnywhere, category = "Greeting", default = true, setter)]
    loud: bool,
    #[uproperty(EditAnywhere, category = "Greeting", default = [1.0, 0.5, 0.0])]
    tint: LinearColor,
    #[uproperty(EditAnywhere, category = "Greeting", default = Excited)]
    mood: Mood,
    #[uproperty(EditAnywhere, category = "Greeting")]
    texture: Option<Obj<Texture>>,
    #[uproperty(EditAnywhere, category = "Greeting")]
    textures: Vec<Obj<Texture>>,
    /// Rust state: greetings so far.
    greeted: u32,
    changes: Vec<GreeterProperty>,
}

#[uclass_impl]
impl Greeter {
    #[init]
    fn start(&mut self) {
        self.greeted = 100;
    }

    /// A greeting, counted.
    #[ufunction(BlueprintCallable, category = "Greeting")]
    fn greet(&mut self, punctuation: &str) -> String {
        self.greeted += 1;
        let text = format!("Hello, {}{punctuation} #{}", self.name(), self.greeted);
        if self.loud() {
            text.to_uppercase()
        } else {
            text
        }
    }

    #[ufunction(BlueprintPure, category = "Greeting")]
    fn size(&self, extra: Vector2) -> Vector2 {
        let s = f64::from(self.scale());
        Vector2::new(s + extra.x, s * 2.0 + extra.y)
    }

    #[ufunction(BlueprintCallable, category = "Greeting")]
    fn add(a: i32, b: i32) -> i32 {
        a + b
    }

    #[ufunction(BlueprintCallable, category = "Greeting")]
    fn next_mood(&mut self) -> Mood {
        let next = match self.mood() {
            Mood::Calm => Mood::Excited,
            Mood::Excited => Mood::Sleepy,
            Mood::Sleepy => Mood::Calm,
        };
        self.set_mood(next);
        next
    }

    #[ufunction(BlueprintCallable, category = "Greeting")]
    fn brighter(&self, amount: f32) -> LinearColor {
        let c = self.tint();
        LinearColor::new(c.r + amount, c.g + amount, c.b + amount, c.a)
    }

    #[ufunction(BlueprintCallable, category = "Greeting")]
    fn count_textures(&self) -> i32 {
        self.textures().len() as i32 + i32::from(self.texture().is_some())
    }

    #[ufunction(BlueprintCallable, category = "Greeting")]
    fn keep_first_texture(&mut self) {
        let first: Vec<Obj<Texture>> = self.textures().into_iter().take(1).collect();
        self.set_textures(&first);
        self.set_texture(first.first().copied());
    }

    #[ufunction(BlueprintCallable, category = "Greeting")]
    fn sum(&self, values: &[f32]) -> f32 {
        values.iter().sum()
    }

    /// The parent's bytes, doubled.
    #[ufunction(BlueprintCallable, category = "Greeting")]
    fn doubled(&self) -> Vec<f32> {
        self.bytes().iter().map(|&b| f32::from(b) * 2.0).collect()
    }

    #[ufunction(BlueprintCallable, category = "Greeting")]
    fn ping(&self) -> i32 {
        self.notify(&format!("ping from {}", self.label()), &[1.0, 2.0, 3.5])
    }

    /// Overrides `UHelloBase::OnTick(float)`, after the parent's.
    #[uoverride]
    fn on_tick(&mut self, delta: f32) {
        self.set_scale(self.scale() + delta);
    }

    /// Overrides `UHelloBase::Describe() const`.
    #[uoverride(const)]
    fn describe(&self) -> String {
        format!("{} greetings, mood {:?}", self.greeted, self.mood())
    }

    #[on_changed]
    fn changed(&mut self, property: GreeterProperty) {
        self.changes.push(property);
    }

    #[ufunction(BlueprintCallable, category = "Greeting")]
    fn change_count(&self) -> i32 {
        self.changes.len() as i32
    }

    #[ufunction(BlueprintCallable, category = "Greeting")]
    fn panics(&mut self) -> i32 {
        panic!("on purpose")
    }
}

/// `Ferreal::HelloRust::Shout(...)` from C++.
#[export]
pub fn shout(text: &str, notifier: Option<Obj<Notifier>>) -> String {
    let loud = text.to_uppercase();
    if let Some(n) = notifier {
        n.notify(&loud, &[]);
    }
    loud
}

/// `Ferreal::HelloRust::Poke(...)` from C++: calls engine functions through the generated
/// bindings (in the tests, the mock engine's), and reports what they returned.
#[export]
pub fn poke(actor: Obj<engine::Actor>, component: Obj<engine::PrimitiveComponent>) -> String {
    use engine::{
        ActorApi, CollisionEnabled, KismetStringLibrary, KismetSystemLibrary,
        PrimitiveComponentApi, SceneComponentApi,
    };

    actor.set_actor_hidden_in_game(true);
    actor.set_owner(Some(actor));
    let own = actor.get_owner() == Some(actor);
    let turned = actor.set_actor_rotation(Rotator::new(10.0, 20.0, 30.0), false);
    let at = actor.get_actor_location();
    let tagged = actor.actor_has_tag("Rusty");
    component.set_visibility(false, true);
    component.set_collision_enabled(CollisionEnabled::QUERY_AND_PHYSICS);
    let collision = component.get_collision_enabled();
    // Declared with a float by the bindings, a double by the (mock) engine: disabled, 0.
    let age = actor.get_game_time_since_creation();
    let number = KismetStringLibrary::conv_int_to_string(collision.0);
    KismetStringLibrary::concat_str_str(
        &KismetSystemLibrary::get_display_name(Some(actor)),
        &format!(
            " own={own} turned={turned} z={} tagged={tagged} collision={number} age={age}",
            at.z
        ),
    )
}
