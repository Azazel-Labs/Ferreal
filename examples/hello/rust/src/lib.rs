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
    UKismetSystemLibrary,
    UKismetMathLibrary,
    UGameplayStatics
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
    // No owner (an actor cannot own itself).
    actor.set_owner(None::<Obj<engine::Actor>>);
    let own = actor.get_owner().is_none();
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

/// `Ferreal::HelloRust::Label(...)` from C++: C++ written inline with `cpp!`, for what the
/// engine bindings cannot reach (they call reflected functions only).
#[export]
pub fn label(actor: Obj<engine::Actor>, times: i32) -> String {
    // A plain C++ method of AActor, not a UFUNCTION (in another macro's arguments, too).
    let name = format!(
        "{} x",
        cpp!([actor: Obj<engine::Actor>] -> String {
            return actor->GetActorNameOrLabel();
        })
    );
    let most = cpp!([times: i32] -> i32 { return FMath::Max(times, 2); });
    cpp!([name: &str, most: i32] -> String {
        return FString::Printf(TEXT("%s%d"), *name, most);
    })
}

/// `Ferreal::HelloRust::Hit(...)` from C++: engine structs, copied to Rust: an `FHitResult`
/// the engine makes, and the actor's `FTransform` (a `const FTransform&` in C++).
#[export]
pub fn hit(actor: Obj<engine::Actor>, component: Obj<engine::PrimitiveComponent>) -> String {
    use engine::{ActorApi, GameplayStatics, PhysicalMaterial};

    let at = Vector3::new(1.0, 2.0, 3.0);
    let zero = Vector3::ZERO;
    let hit = GameplayStatics::make_hit_result(
        true,
        false,
        0.5,
        10.0,
        at,
        at,
        zero,
        zero,
        None::<Obj<PhysicalMaterial>>,
        Some(actor),
        Some(component),
        "Spine",
        "",
        0,
        0,
        0,
        zero,
        at,
    );
    let transform = actor.get_transform();
    // The same types through C++ written inline, both ways.
    let later = cpp!([hit: engine::HitResult] -> engine::HitResult {
        FHitResult Later = hit;
        Later.Time += 1.0f;
        return Later;
    });
    let moved = cpp!([transform: Transform] -> Transform {
        const FVector At = transform.GetTranslation();
        return FTransform(transform.GetRotation(), FVector(At.X, At.Y, At.Z + 10), transform.GetScale3D());
    });
    let mode = engine::CollisionEnabled::QUERY_ONLY;
    // An engine enum is its own C++ type in the block (`ECollisionEnabled::Type`).
    let next = cpp!([mode: engine::CollisionEnabled] -> i32 {
        return mode == ECollisionEnabled::QueryOnly ? 2 : 0;
    });
    format!(
        "blocking={} time={} z={} component={} actor={} bone={} scale={} turn={} later={} {} moved={} next={}",
        hit.blocking_hit,
        hit.time,
        hit.location.z,
        hit.component == Some(component),
        hit.hit_object_handle == Some(actor) && later.hit_object_handle == Some(actor),
        hit.bone_name,
        transform.scale.x,
        transform.rotation.z,
        later.time,
        later.bone_name,
        moved.translation.z,
        next
    )
}

/// `Ferreal::HelloRust::Sweep(...)` from C++: output parameters. The engine fills
/// `FHitResult& SweepHitResult`, which Rust gets back with the return value; it reads and
/// changes `UPARAM(ref) FVector& A`, a `&mut` in Rust.
#[export]
pub fn sweep(actor: Obj<engine::Actor>) -> String {
    use engine::{ActorApi, KismetMathLibrary};

    let (moved, swept) = actor.set_actor_location(Vector3::new(0.0, 0.0, 7.0), true, false);
    let mut direction = Vector3::new(3.0, 0.0, 4.0);
    KismetMathLibrary::vector_normalize(&mut direction, 0.0001);
    format!(
        "moved={moved} swept={} actor={} x={:.3}",
        swept.time,
        swept.hit_object_handle == Some(actor),
        direction.x
    )
}

/// `Ferreal::HelloRust::Trace(...)` from C++: arrays. Strings returned (a `Vec`) and passed
/// (a slice); a line trace that ignores the actors it is given, and fills an `FHitResult`.
#[export]
pub fn trace(actor: Obj<engine::Actor>) -> String {
    use engine::{DrawDebugTrace, KismetStringLibrary, KismetSystemLibrary, TraceTypeQuery};

    let parts = KismetStringLibrary::parse_into_array("a,b,,c", ",", true);
    let joined = KismetStringLibrary::join_string_array(&["x", "y"], "+");
    let line = |ignore: &[Obj<engine::Actor>]| {
        KismetSystemLibrary::line_trace_single(
            Some(actor),
            Vector3::ZERO,
            Vector3::new(0.0, 0.0, -100.0),
            TraceTypeQuery::TRACE_TYPE_QUERY1,
            false,
            ignore,
            DrawDebugTrace::NONE,
            false,
            LinearColor::new(1.0, 0.0, 0.0, 1.0),
            LinearColor::new(0.0, 1.0, 0.0, 1.0),
            0.0,
        )
    };
    let (hit, result) = line(&[]);
    let (ignored, _) = line(&[actor]);
    format!(
        "parts={parts:?} joined={joined} hit={hit} z={} actor={} ignored={ignored}",
        result.location.z,
        result.hit_object_handle == Some(actor)
    )
}
