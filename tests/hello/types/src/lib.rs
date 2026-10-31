//! Engine types shared by Rust crates: those of the classes named below (their markers, and
//! the structs and enums their functions pass). Every crate depending on this one uses them
//! as its own, so a crate can hand another an `Obj<engine::Actor>` or an `engine::HitResult`.
//! It binds no functions: each crate binds those it calls.

ferreal::engine!(AActor, UPrimitiveComponent);
ferreal::engine_types!();

use ferreal::Obj;

/// What a hit says, for any crate's hits.
pub fn describe(hit: &engine::HitResult) -> String {
    format!(
        "blocking={} time={} bone={}",
        hit.blocking_hit, hit.time, hit.bone_name
    )
}

/// Whether the hit is of `actor`.
pub fn hit_actor(hit: &engine::HitResult, actor: Obj<engine::Actor>) -> bool {
    hit.hit_object_handle == Some(actor)
}
