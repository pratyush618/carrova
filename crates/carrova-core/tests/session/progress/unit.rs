use super::*;
use crate::session::VecSink;

fn reports(sink: &VecSink) -> Vec<(u64, u64)> {
    sink.events()
        .into_iter()
        .filter_map(|e| match e {
            Event::Progress { done, total, .. } => Some((done, total)),
            _ => None,
        })
        .collect()
}

#[test]
fn reports_once_per_step() {
    let sink = VecSink::default();
    let mut m = ProgressMeter::new(&sink, Phase::Transfer, 10, 4).start();
    for _ in 0..5 {
        m.add(2);
    }
    m.finish();
    assert_eq!(reports(&sink), [(0, 10), (4, 10), (8, 10), (10, 10)]);
}

#[test]
fn every_update_reports_each_add() {
    let sink = VecSink::default();
    let mut m = ProgressMeter::new(&sink, Phase::Archive, 3, ProgressMeter::EVERY_UPDATE);
    m.add(1);
    m.add(2);
    assert_eq!(reports(&sink), [(1, 3), (3, 3)]);
}

#[test]
fn resumed_meter_counts_on_from_earlier_runs() {
    let sink = VecSink::default();
    let mut m = ProgressMeter::new(&sink, Phase::Send, 100, 10).resume(50);
    m.add(9);
    m.add(1);
    assert_eq!(reports(&sink), [(60, 100)]);
}

#[test]
fn estimated_total_never_reports_less_than_done() {
    let sink = VecSink::default();
    let mut m =
        ProgressMeter::new(&sink, Phase::Capture, 5, ProgressMeter::EVERY_UPDATE).estimated();
    m.add(8);
    assert_eq!(reports(&sink), [(8, 8)]);
}

#[test]
fn complete_reports_the_total() {
    let sink = VecSink::default();
    let mut m = ProgressMeter::new(&sink, Phase::Stage, 7, PROGRESS_STEP);
    m.add(3);
    m.complete();
    assert_eq!(reports(&sink), [(7, 7)]);
}
