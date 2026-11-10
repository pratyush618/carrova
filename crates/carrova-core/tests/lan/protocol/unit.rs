use super::*;

fn inventory() -> Inventory {
    Inventory {
        os: carrova_platform::OsKind::Linux,
        carrova_version: "0".into(),
        apps: Vec::new(),
        free_space: None,
        device_name: None,
    }
}

#[test]
fn fresh_pairing_walks_inventory_plan_fetch_applied() {
    let (s, a) = SendStep::AwaitInventory
        .on(LanMsg::Inventory(inventory()))
        .unwrap();
    assert_eq!(
        (s, a),
        (SendStep::AwaitPlan, SendAction::SendManifest(inventory()))
    );
    let (s, _) = s.on(LanMsg::PlanSummary(PlanSummary::default())).unwrap();
    assert_eq!(s, SendStep::Serving);
    assert_eq!(s.on(LanMsg::Fetch).unwrap(), (s, SendAction::Serve));
    let (_, a) = s.on(LanMsg::Applied(Checklist::default())).unwrap();
    assert_eq!(a, SendAction::Finished(Checklist::default()));
}

#[test]
fn resumed_connection_starts_where_the_receiver_says() {
    let (s, _) = SendStep::AwaitResume
        .on(LanMsg::Resume {
            stage: ReceiverStage::Transferring,
        })
        .unwrap();
    assert_eq!(s, SendStep::Serving);
    let (s, _) = SendStep::AwaitResume
        .on(LanMsg::Resume {
            stage: ReceiverStage::Planning,
        })
        .unwrap();
    assert_eq!(s, SendStep::AwaitPlan);
}

#[test]
fn out_of_order_messages_are_protocol_errors() {
    assert!(SendStep::AwaitInventory.on(LanMsg::Fetch).is_err());
    assert!(SendStep::AwaitPlan.on(LanMsg::Fetch).is_err());
    assert!(
        SendStep::AwaitResume
            .on(LanMsg::Inventory(inventory()))
            .is_err()
    );
    assert!(
        SendStep::Serving
            .on(LanMsg::Applied(Checklist::default()))
            .is_ok()
    );
    assert!(
        SendStep::AwaitPlan
            .on(LanMsg::Applied(Checklist::default()))
            .is_err()
    );
}

#[test]
fn protocol_error_texts_read_as_before() {
    let err = SendStep::AwaitPlan.on(LanMsg::Fetch).unwrap_err();
    assert_eq!(
        err.to_string(),
        "protocol error: unexpected fetch while AwaitPlan"
    );
    for (v, text) in [
        (
            LanViolation::ManifestTooLarge(9),
            "manifest of 9 bytes exceeds the limit",
        ),
        (
            LanViolation::ManifestLengthMismatch,
            "manifest parts disagree on its length",
        ),
        (LanViolation::ManifestPartTooLong, "manifest part too long"),
        (
            LanViolation::ExpectedManifest("fetch"),
            "expected the manifest, got fetch",
        ),
    ] {
        assert_eq!(
            CoreError::from(v).to_string(),
            format!("protocol error: {text}")
        );
    }
}

#[test]
fn abort_is_accepted_in_every_step() {
    for s in [
        SendStep::AwaitInventory,
        SendStep::AwaitResume,
        SendStep::AwaitPlan,
        SendStep::Serving,
    ] {
        let (_, a) = s
            .on(LanMsg::Abort {
                reason: AbortReason::DryRun,
            })
            .unwrap();
        assert_eq!(a, SendAction::Aborted(AbortReason::DryRun));
    }
}

#[test]
fn manifest_parts_reassemble_to_the_original_bytes() {
    let bytes: Vec<u8> = (0..(MANIFEST_PART_LEN * 2 + 7)).map(|i| i as u8).collect();
    let parts: Vec<LanMsg> = manifest_messages(&bytes).collect();
    assert_eq!(parts.len(), 3);
    let mut joined = Vec::new();
    for p in parts {
        let LanMsg::Manifest { total_len, part } = p else {
            panic!("not a manifest part");
        };
        assert_eq!(total_len, bytes.len() as u64);
        joined.extend(part);
    }
    assert_eq!(joined, bytes);
    assert_eq!(manifest_messages(&[]).count(), 1);
}
