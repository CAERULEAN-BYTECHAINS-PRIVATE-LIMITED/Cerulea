#![no_main]
//! The Blueprint decode path.
//!
//! A deployment installs a Blueprint. The artefact arrives as bytes and
//! decodes into the twelve sections before anything runs. The target drives
//! both forms Cerulea accepts. The encoded form and the text form.

use cerulea_blueprint::Blueprint;
use codec::Decode;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(blueprint) = Blueprint::decode(&mut &data[..]) {
        let _ = blueprint.validate();
    }
    if let Ok(text) = core::str::from_utf8(data) {
        if let Ok(blueprint) = serde_json::from_str::<Blueprint>(text) {
            let _ = blueprint.validate();
        }
    }
});
