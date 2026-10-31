//! The FerrealSample plugin, in Rust: orbs to collect, and the rules of collecting them.
//!
//! * `AOrb`, an actor: it circles the spot it was spawned (or placed) at, bobs, spins and
//!   glows. Its C++ parent, `AOrbBase` (../Source/FerrealSample), gives it its sphere and ticks.
//! * `UOrbGame`, the rules: a world subsystem, which the engine makes in every game world by
//!   itself. It spawns a ring of orbs around the player, collects those the player walks
//!   into, keeps the score on screen, and starts a bigger round once all are collected. The
//!   game's C++ calls its functions (`Round`, `OrbsLeft`, `CollectAll`), as it would a C++
//!   plugin's.

use std::f64::consts::TAU;

use ferreal::prelude::*;

ferreal::module!();

ferreal::engine!(
    AActor,
    UMeshComponent,
    UGameplayStatics,
    UKismetSystemLibrary,
    UActorComponent
);

use engine::{ActorApi, GameplayStatics, KismetSystemLibrary, MeshComponentApi};

/// An orb to collect: circles where it started, bobbing and spinning.
#[uclass(
    parent = "AOrbBase",
    include = "OrbBase.h",
    Blueprintable,
    meta(DisplayName = "Rust Orb")
)]
pub struct Orb {
    /// How far from where it started it circles, in centimeters.
    #[uproperty(EditAnywhere, BlueprintReadWrite, category = "Orb", default = 60.0)]
    radius: f32,
    /// How fast it circles, in turns per second.
    #[uproperty(EditAnywhere, BlueprintReadWrite, category = "Orb", default = 0.25)]
    speed: f32,
    /// Its color; a hue of its own, from where it is, when `rainbow` is set.
    #[uproperty(EditAnywhere, BlueprintReadWrite, category = "Orb", default = [1.0, 0.45, 0.1, 1.0])]
    color: LinearColor,
    #[uproperty(EditAnywhere, BlueprintReadWrite, category = "Orb", default = true)]
    rainbow: bool,
    /// Rust state: where it started, and for how long it has been circling.
    home: Vector3,
    age: f64,
    phase: f64,
}

#[uclass_impl]
impl Orb {
    /// Overrides `AActor::BeginPlay`, after the parent's.
    #[uoverride]
    fn begin_play(&mut self) {
        self.home = self.get_actor_location();
        // Orbs side by side start their circles apart.
        self.phase = self.home.y.atan2(self.home.x);
        let color = if self.rainbow() {
            hue(self.phase / TAU)
        } else {
            self.color()
        };
        if let Some(mesh) = self.get_root_component() {
            mesh.cast_unchecked::<engine::MeshComponent>()
                .set_vector_parameter_value_on_materials(
                    "Color",
                    Vector3::new(color.r.into(), color.g.into(), color.b.into()),
                );
        }
    }

    /// Overrides `AActor::Tick`, after the parent's.
    #[uoverride]
    fn tick(&mut self, delta: f32) {
        self.age += f64::from(delta);
        let turn = self.phase + self.age * f64::from(self.speed()) * TAU;
        let r = f64::from(self.radius());
        let bob = (self.age * 3.0 + self.phase).sin() * 15.0;
        let at = self.home + Vector3::new(turn.cos() * r, turn.sin() * r, bob);
        self.set_actor_location(at, false, true);
        self.set_actor_rotation(Rotator::new(0.0, self.age * 120.0, 0.0), true);
    }
}

/// The game's rules: rounds of orbs to collect.
#[uclass(parent = "UOrbGameBase", include = "OrbGameBase.h")]
pub struct OrbGame {
    /// How close the player must come to an orb to collect it, in centimeters.
    #[uproperty(EditAnywhere, category = "Orbs", default = 150.0)]
    reach: f32,
    round: u32,
    left: u32,
    total: u32,
    started: f64,
    /// When the next round starts (after the last, or at once for the first).
    next_round: Option<f64>,
    /// `-OrbAutopilot` on the command line: the player is taken from orb to orb (for tests,
    /// and to watch), next at `next_hop`.
    autopilot: bool,
    next_hop: f64,
}

#[uclass_impl]
impl OrbGame {
    /// Overrides `UOrbGameBase::StartGame`: the world has begun play.
    #[uoverride]
    fn start_game(&mut self) {
        self.next_round = Some(0.0);
        self.autopilot = cpp!([] -> bool {
            return FParse::Param(FCommandLine::Get(), TEXT("OrbAutopilot"));
        });
        self.say(
            "Ferreal: this game's rules are Rust. Find the orbs!",
            6.0,
            "Intro",
        );
    }

    /// Overrides `UOrbGameBase::TickGame`: every frame of play.
    #[uoverride]
    fn tick_game(&mut self, _delta: f32) {
        let me = self.obj();
        let now = GameplayStatics::get_time_seconds(Some(me));
        let Some(player) = GameplayStatics::get_player_pawn(Some(me), 0) else {
            return;
        };

        if let Some(when) = self.next_round {
            if now >= when {
                self.next_round = None;
                self.spawn_round(player.get_actor_location(), now);
            }
            return;
        }

        let orbs: Vec<_> =
            GameplayStatics::get_all_actors_of_class(Some(me), ClassOf::<Orb>::get())
                .into_iter()
                .flatten()
                .collect();
        if self.autopilot && now >= self.next_hop {
            if let Some(orb) = orbs.first() {
                player.set_actor_location(orb.get_actor_location(), false, true);
            }
            self.next_hop = now + 0.75;
        }
        let at = player.get_actor_location();
        let reach = f64::from(self.reach());
        for orb in orbs {
            if orb.get_actor_location().distance(at) < reach {
                orb.destroy_actor();
                self.left = self.left.saturating_sub(1);
                self.score();
            }
        }
        if self.left == 0 {
            let took = now - self.started;
            self.say(
                &format!(
                    "Round {} done: {} orbs in {took:.1} s. The next one is bigger.",
                    self.round, self.total
                ),
                4.0,
                "Round",
            );
            self.next_round = Some(now + 4.0);
        }
    }

    /// The round being played; 0 before the first.
    #[ufunction(BlueprintPure, category = "Orbs")]
    fn round(&self) -> i32 {
        i32::try_from(self.round).unwrap_or(i32::MAX)
    }

    /// The orbs of the round left to collect.
    #[ufunction(BlueprintPure, category = "Orbs")]
    fn orbs_left(&self) -> i32 {
        i32::try_from(self.left).unwrap_or(i32::MAX)
    }

    /// Collects every orb left in the round, as if the player had walked into each.
    #[ufunction(BlueprintCallable, category = "Orbs")]
    fn collect_all(&mut self) {
        let me = self.obj();
        for orb in GameplayStatics::get_all_actors_of_class(Some(me), ClassOf::<Orb>::get())
            .into_iter()
            .flatten()
        {
            orb.destroy_actor();
        }
        self.left = 0;
        self.score();
    }
}

impl OrbGame {
    /// A ring of orbs around `center`, more each round.
    fn spawn_round(&mut self, center: Vector3, now: f64) {
        self.round += 1;
        self.total = 4 + self.round * 2;
        self.started = now;
        let me = self.obj();
        let offset = f64::from(self.round) * 0.7;
        let mut spawned = 0;
        for i in 0..self.total {
            let angle = offset + TAU * f64::from(i) / f64::from(self.total);
            let distance = 450.0 + 250.0 * f64::from(i % 3);
            let at = center + Vector3::new(angle.cos() * distance, angle.sin() * distance, 0.0);
            // Spawning is C++ the bindings leave out (Blueprints' spawn nodes are internal).
            let orb = cpp!([me: Obj<OrbGame>, at: Vector3] -> Option<Obj<Orb>> {
                FActorSpawnParameters Params;
                Params.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;
                return me->GetWorld()->SpawnActor<AOrb>(at, FRotator::ZeroRotator, Params);
            });
            spawned += u32::from(orb.is_some());
        }
        self.left = spawned;
        self.total = spawned;
        self.say(
            &format!("Round {}: {spawned} orbs around you.", self.round),
            4.0,
            "Round",
        );
        self.score();
    }

    fn score(&self) {
        self.say(
            &format!("Orbs: {}/{}", self.total - self.left, self.total),
            1.0e6,
            "Score",
        );
    }

    /// Prints on screen (and to the log), replacing the last message of the same `key`.
    fn say(&self, text: &str, seconds: f32, key: &str) {
        KismetSystemLibrary::print_string(
            Some(self.obj()),
            text,
            true,
            true,
            LinearColor::new(0.3, 0.9, 1.0, 1.0),
            seconds,
            key,
        );
    }
}

/// A saturated color of hue `h` (turns).
fn hue(h: f64) -> LinearColor {
    let h = h.rem_euclid(1.0) * 6.0;
    let x = (1.0 - (h % 2.0 - 1.0).abs()) as f32;
    let (r, g, b) = match h as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    LinearColor::new(r, g, b, 1.0)
}
