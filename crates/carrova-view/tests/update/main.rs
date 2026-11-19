//! App update download: what the user sees of it, and what it waits for.
#![allow(clippy::unwrap_used)]

use std::time::{Duration, Instant};

use carrova_view::update::{
    DownloadMeter, DownloadView, REPORT_INTERVAL, UpdateBlocker, update_blocker,
};

const MB: u64 = 1_000_000;

/// Feeds `chunk` bytes of `total` to `meter`, `after` past `start`.
fn feed(
    meter: &mut DownloadMeter,
    start: Instant,
    after: Duration,
    chunk: u64,
    total: Option<u64>,
) -> Option<DownloadView> {
    meter.record(chunk, total, start + after)
}

#[test]
fn the_first_chunk_reports_at_once_without_a_speed() {
    let mut meter = DownloadMeter::default();
    let view = meter.record(MB, Some(100 * MB), Instant::now()).unwrap();
    assert_eq!(
        view,
        DownloadView {
            done: MB,
            total: Some(100 * MB),
            percent: Some(1),
            bytes_per_sec: None,
            secs_left: None,
        }
    );
}

#[test]
fn reports_at_most_four_times_a_second() {
    let start = Instant::now();
    let mut meter = DownloadMeter::default();
    let total = Some(100 * MB);
    assert!(feed(&mut meter, start, Duration::ZERO, MB, total).is_some());
    assert!(feed(&mut meter, start, Duration::from_millis(100), MB, total).is_none());
    assert!(feed(&mut meter, start, Duration::from_millis(200), MB, total).is_none());
    let due = feed(&mut meter, start, REPORT_INTERVAL, MB, total).unwrap();
    assert_eq!(due.done, 4 * MB, "skipped chunks still count");
}

#[test]
fn the_last_chunk_always_reports() {
    let start = Instant::now();
    let mut meter = DownloadMeter::default();
    feed(&mut meter, start, Duration::ZERO, MB, Some(2 * MB));
    let last = feed(
        &mut meter,
        start,
        Duration::from_millis(10),
        MB,
        Some(2 * MB),
    )
    .unwrap();
    assert_eq!(last.percent, Some(100));
    assert_eq!(last.secs_left, Some(0));
}

#[test]
fn a_steady_speed_gives_its_rate_and_the_time_left() {
    let start = Instant::now();
    let mut meter = DownloadMeter::default();
    let total = Some(100 * MB);
    let mut view = feed(&mut meter, start, Duration::ZERO, 0, total);
    // 4 MB/s: 1 MB every 250 ms, for 10 s.
    for tick in 1..=40 {
        view = feed(&mut meter, start, REPORT_INTERVAL * tick, MB, total);
    }
    let view = view.unwrap();
    assert_eq!(view.done, 40 * MB);
    assert_eq!(view.bytes_per_sec, Some(4 * MB));
    assert_eq!(view.secs_left, Some(15));
}

#[test]
fn the_speed_follows_a_change_smoothly() {
    let start = Instant::now();
    let mut meter = DownloadMeter::default();
    let total = Some(1000 * MB);
    feed(&mut meter, start, Duration::ZERO, 0, total);
    let mut at = Duration::ZERO;
    for _ in 0..40 {
        at += REPORT_INTERVAL;
        feed(&mut meter, start, at, MB, total); // 4 MB/s
    }
    at += REPORT_INTERVAL;
    let first = feed(&mut meter, start, at, 2 * MB, total).unwrap(); // a burst at 8 MB/s
    let speed = first.bytes_per_sec.unwrap();
    assert!(
        speed > 4 * MB && speed < 6 * MB,
        "one burst moves the speed only part way: {speed}"
    );
    for _ in 0..40 {
        at += REPORT_INTERVAL;
        feed(&mut meter, start, at, 2 * MB, total);
    }
    at += REPORT_INTERVAL;
    let settled = feed(&mut meter, start, at, 2 * MB, total).unwrap();
    let speed = settled.bytes_per_sec.unwrap();
    assert!(
        speed > 7_900_000 && speed <= 8 * MB,
        "ten seconds later it reads the new speed: {speed}"
    );
}

#[test]
fn a_crawling_download_has_no_time_left() {
    let start = Instant::now();
    let mut meter = DownloadMeter::default();
    let total = Some(100 * MB);
    feed(&mut meter, start, Duration::ZERO, MB, total);
    let mut view = None;
    for tick in 1..=30 {
        view = feed(&mut meter, start, Duration::from_secs(tick), 100, total).or(view);
    }
    assert_eq!(view.unwrap().secs_left, None);
}

#[test]
fn an_unknown_size_has_no_percent_or_time_left() {
    let start = Instant::now();
    let mut meter = DownloadMeter::default();
    feed(&mut meter, start, Duration::ZERO, MB, None);
    let view = feed(&mut meter, start, Duration::from_secs(1), MB, None).unwrap();
    assert_eq!(view.total, None);
    assert_eq!(view.percent, None);
    assert_eq!(view.secs_left, None);
    assert_eq!(view.bytes_per_sec, Some(MB));
}

#[test]
fn a_move_holds_the_install_before_a_call() {
    assert_eq!(update_blocker(false, false), None);
    assert_eq!(update_blocker(true, false), Some(UpdateBlocker::Move));
    assert_eq!(update_blocker(false, true), Some(UpdateBlocker::Call));
    assert_eq!(update_blocker(true, true), Some(UpdateBlocker::Move));
}
