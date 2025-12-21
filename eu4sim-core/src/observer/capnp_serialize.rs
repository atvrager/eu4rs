//! Cap'n Proto serialization for training data.
//!
//! Converts Rust training samples to Cap'n Proto binary format for
//! efficient consumption by Python ML pipelines.
//!
//! Files use the `.cpb` extension (Cap'n Proto Binary).

use crate::ai::VisibleWorldState;
use crate::fixed::Fixed;
use crate::input::{Command, DevType};
use crate::observer::datagen::TrainingSample;
use crate::state::TechType;
use crate::training_capnp;
use capnp::message::Builder;
use capnp::serialize_packed;
use std::io::Write;

/// Serialize a batch of training samples to Cap'n Proto format.
///
/// Writes a `TrainingBatch` message containing all samples for a given year.
pub fn serialize_batch<W: Write>(
    writer: &mut W,
    year: i16,
    samples: &[TrainingSample],
) -> Result<(), capnp::Error> {
    let mut message = Builder::new_default();
    let mut batch = message.init_root::<training_capnp::training_batch::Builder>();

    batch.set_year(year);
    let mut sample_list = batch.init_samples(samples.len() as u32);

    for (i, sample) in samples.iter().enumerate() {
        let mut s = sample_list.reborrow().get(i as u32);
        write_sample(&mut s, sample)?;
    }

    serialize_packed::write_message(writer, &message)
}

/// Serialize a complete training file with multiple batches.
pub fn serialize_file<W: Write>(
    writer: &mut W,
    batches: &[(i16, Vec<TrainingSample>)],
) -> Result<(), capnp::Error> {
    let mut message = Builder::new_default();
    let mut file = message.init_root::<training_capnp::training_file::Builder>();

    file.set_schema_version(1);
    let mut batch_list = file.init_batches(batches.len() as u32);

    for (i, (year, samples)) in batches.iter().enumerate() {
        let mut batch = batch_list.reborrow().get(i as u32);
        batch.set_year(*year);
        let mut sample_list = batch.init_samples(samples.len() as u32);

        for (j, sample) in samples.iter().enumerate() {
            let mut s = sample_list.reborrow().get(j as u32);
            write_sample(&mut s, sample)?;
        }
    }

    serialize_packed::write_message(writer, &message)
}

fn write_sample(
    s: &mut training_capnp::training_sample::Builder,
    sample: &TrainingSample,
) -> Result<(), capnp::Error> {
    s.set_tick(sample.tick);
    s.set_country(&sample.country);
    s.set_chosen_action(sample.chosen_action);

    // Write visible state
    let mut state = s.reborrow().init_state();
    write_visible_state(&mut state, &sample.state)?;

    // Write available commands
    let mut cmds = s
        .reborrow()
        .init_available_commands(sample.available_commands.len() as u32);
    for (i, cmd) in sample.available_commands.iter().enumerate() {
        let mut c = cmds.reborrow().get(i as u32);
        write_command(&mut c, cmd)?;
    }

    // Write chosen command if present
    if let Some(ref cmd) = sample.chosen_command {
        let mut chosen = s.reborrow().init_chosen_command();
        write_command(&mut chosen, cmd)?;
    }

    Ok(())
}

fn write_visible_state(
    state: &mut training_capnp::visible_world_state::Builder,
    vs: &VisibleWorldState,
) -> Result<(), capnp::Error> {
    // Date
    let mut date = state.reborrow().init_date();
    date.set_year(vs.date.year as i16);
    date.set_month(vs.date.month);
    date.set_day(vs.date.day);

    state.set_observer(&vs.observer);
    state.set_at_war(vs.at_war);

    // Own country state
    let mut own = state.reborrow().init_own_country();
    write_fixed(&mut own.reborrow().init_treasury(), vs.own_country.treasury);
    write_fixed(&mut own.reborrow().init_manpower(), vs.own_country.manpower);
    own.set_stability(vs.own_country.stability.get() as i8);
    write_fixed(
        &mut own.reborrow().init_prestige(),
        vs.own_country.prestige.get(),
    );
    write_fixed(
        &mut own.reborrow().init_army_tradition(),
        vs.own_country.army_tradition.get(),
    );
    write_fixed(&mut own.reborrow().init_adm_mana(), vs.own_country.adm_mana);
    write_fixed(&mut own.reborrow().init_dip_mana(), vs.own_country.dip_mana);
    write_fixed(&mut own.reborrow().init_mil_mana(), vs.own_country.mil_mana);
    own.set_adm_tech(vs.own_country.adm_tech);
    own.set_dip_tech(vs.own_country.dip_tech);
    own.set_mil_tech(vs.own_country.mil_tech);

    // Religion (empty string if None)
    own.set_religion(vs.own_country.religion.as_deref().unwrap_or(""));

    // Embraced institutions
    let institutions: Vec<_> = vs
        .own_country
        .embraced_institutions
        .iter()
        .map(|id| id.as_str())
        .collect();
    let mut inst_list = own
        .reborrow()
        .init_embraced_institutions(institutions.len() as u32);
    for (i, inst) in institutions.iter().enumerate() {
        inst_list.set(i as u32, inst);
    }

    // Known countries
    let mut known = state
        .reborrow()
        .init_known_countries(vs.known_countries.len() as u32);
    for (i, country) in vs.known_countries.iter().enumerate() {
        known.set(i as u32, country);
    }

    // Enemy provinces
    let enemy_list: Vec<_> = vs.enemy_provinces.iter().copied().collect();
    let mut enemies = state
        .reborrow()
        .init_enemy_provinces(enemy_list.len() as u32);
    for (i, &prov) in enemy_list.iter().enumerate() {
        enemies.set(i as u32, prov);
    }

    // Country strength
    let strength_list: Vec<_> = vs.known_country_strength.iter().collect();
    let mut strength = state
        .reborrow()
        .init_known_country_strength(strength_list.len() as u32);
    for (i, (country, &str_val)) in strength_list.iter().enumerate() {
        let mut entry = strength.reborrow().get(i as u32);
        entry.set_country(country);
        entry.set_strength(str_val);
    }

    // War scores
    let score_list: Vec<_> = vs.our_war_score.iter().collect();
    let mut scores = state.reborrow().init_our_war_score(score_list.len() as u32);
    for (i, (&war_id, &score)) in score_list.iter().enumerate() {
        let mut entry = scores.reborrow().get(i as u32);
        entry.set_war_id(war_id);
        write_fixed(&mut entry.init_score(), score);
    }

    Ok(())
}

fn write_fixed(builder: &mut training_capnp::fixed::Builder, value: Fixed) {
    builder.set_raw(value.raw());
}

fn write_command(
    cmd: &mut training_capnp::command::Builder,
    command: &Command,
) -> Result<(), capnp::Error> {
    match command {
        Command::Pass => cmd.set_pass(()),
        Command::Quit => cmd.set_quit(()),
        Command::Move {
            army_id,
            destination,
        } => {
            let mut m = cmd.reborrow().init_move();
            m.set_army_id(*army_id);
            m.set_destination(*destination);
        }
        Command::MoveFleet {
            fleet_id,
            destination,
        } => {
            let mut m = cmd.reborrow().init_move_fleet();
            m.set_fleet_id(*fleet_id);
            m.set_destination(*destination);
        }
        Command::DeclareWar { target, cb } => {
            let mut m = cmd.reborrow().init_declare_war();
            m.set_target(target);
            m.set_cb(cb.as_deref().unwrap_or(""));
        }
        Command::AcceptPeace { war_id } => cmd.set_accept_peace(*war_id),
        Command::RejectPeace { war_id } => cmd.set_reject_peace(*war_id),
        Command::BuyTech { tech_type } => {
            let tt = match tech_type {
                TechType::Adm => training_capnp::TechType::Adm,
                TechType::Dip => training_capnp::TechType::Dip,
                TechType::Mil => training_capnp::TechType::Mil,
            };
            cmd.set_buy_tech(tt);
        }
        Command::EmbraceInstitution { institution } => cmd.set_embrace_institution(institution),
        Command::DevelopProvince { province, dev_type } => {
            let mut d = cmd.reborrow().init_develop_province();
            d.set_province(*province);
            let dt = match dev_type {
                DevType::Tax => training_capnp::DevType::Tax,
                DevType::Production => training_capnp::DevType::Production,
                DevType::Manpower => training_capnp::DevType::Manpower,
            };
            d.set_dev_type(dt);
        }
        Command::StartColony { province } => cmd.set_start_colony(*province),
        Command::AbandonColony { province } => cmd.set_abandon_colony(*province),
        Command::OfferAlliance { target } => cmd.set_offer_alliance(target),
        Command::BreakAlliance { target } => cmd.set_break_alliance(target),
        Command::AcceptAlliance { from } => cmd.set_accept_alliance(from),
        Command::RejectAlliance { from } => cmd.set_reject_alliance(from),
        Command::SetRival { target } => cmd.set_set_rival(target),
        Command::RemoveRival { target } => cmd.set_remove_rival(target),
        // Map remaining commands to Pass (they'll be added as schema evolves)
        _ => cmd.set_pass(()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{CountryState, Date};
    use std::collections::{HashMap, HashSet};

    fn make_test_sample() -> TrainingSample {
        TrainingSample {
            tick: 365,
            country: "FRA".to_string(),
            state: VisibleWorldState {
                date: Date {
                    year: 1445,
                    month: 1,
                    day: 1,
                },
                observer: "FRA".to_string(),
                own_country: CountryState::default(),
                at_war: false,
                known_countries: vec!["ENG".to_string(), "SPA".to_string()],
                enemy_provinces: HashSet::new(),
                known_country_strength: HashMap::new(),
                our_war_score: HashMap::new(),
            },
            available_commands: vec![
                Command::Pass,
                Command::BuyTech {
                    tech_type: TechType::Adm,
                },
            ],
            chosen_action: 0,
            chosen_command: Some(Command::Pass),
        }
    }

    #[test]
    fn test_serialize_batch() {
        let samples = vec![make_test_sample()];
        let mut buf = Vec::new();

        serialize_batch(&mut buf, 1445, &samples).expect("Serialization failed");

        assert!(!buf.is_empty(), "Serialized data should not be empty");

        // Verify we can read it back
        let reader = capnp::serialize_packed::read_message(
            &mut buf.as_slice(),
            capnp::message::ReaderOptions::new(),
        )
        .expect("Failed to read message");

        let batch = reader
            .get_root::<training_capnp::training_batch::Reader>()
            .expect("Failed to get root");
        assert_eq!(batch.get_year(), 1445);
        assert_eq!(batch.get_samples().unwrap().len(), 1);
    }

    #[test]
    fn test_serialize_file() {
        let samples = vec![make_test_sample()];
        let batches = vec![(1445i16, samples)];
        let mut buf = Vec::new();

        serialize_file(&mut buf, &batches).expect("Serialization failed");

        assert!(!buf.is_empty(), "Serialized data should not be empty");

        // Verify we can read it back
        let reader = capnp::serialize_packed::read_message(
            &mut buf.as_slice(),
            capnp::message::ReaderOptions::new(),
        )
        .expect("Failed to read message");

        let file = reader
            .get_root::<training_capnp::training_file::Reader>()
            .expect("Failed to get root");
        assert_eq!(file.get_schema_version(), 1);
        assert_eq!(file.get_batches().unwrap().len(), 1);
    }
}
