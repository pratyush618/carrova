use carrova_link::Feature;
use carrova_platform::phone::Sim;
use carrova_view::phone::{SimChoice, sim_choices};

fn sim(id: i32, slot: u8, sms_default: bool, voice_default: bool) -> Sim {
    Sim {
        id,
        slot,
        name: format!("Carrier {id}"),
        sms_default,
        voice_default,
    }
}

fn defaults(choices: &[SimChoice]) -> Vec<i32> {
    choices.iter().filter(|c| c.default).map(|c| c.id).collect()
}

#[test]
fn one_sim_offers_no_choice() {
    assert!(sim_choices(&[], Feature::Messages).is_empty());
    assert!(sim_choices(&[sim(3, 0, true, true)], Feature::Calls).is_empty());
}

#[test]
fn two_sims_by_slot_with_the_default_for_their_use() {
    // Texts default to slot 2's SIM, calls to slot 1's; listed out of order.
    let sims = [sim(9, 1, true, false), sim(3, 0, false, true)];
    let texts = sim_choices(&sims, Feature::Messages);
    assert_eq!(
        texts.iter().map(|c| (c.id, c.slot)).collect::<Vec<_>>(),
        [(3, 1), (9, 2)]
    );
    assert_eq!(texts[0].name, "Carrier 3");
    assert_eq!(defaults(&texts), [9]);
    assert_eq!(defaults(&sim_choices(&sims, Feature::Calls)), [3]);
}

#[test]
fn ask_every_time_preselects_the_first_slot() {
    let sims = [sim(9, 1, false, false), sim(3, 0, false, false)];
    assert_eq!(defaults(&sim_choices(&sims, Feature::Messages)), [3]);
}

#[test]
fn debug_hides_the_carrier() {
    let choices = sim_choices(
        &[sim(3, 0, true, true), sim(9, 1, false, false)],
        Feature::Calls,
    );
    assert!(!format!("{choices:?}").contains("Carrier"));
}
