#[cfg(test)]
mod tests {
    use super::*;
    use crate::pager::{
        PAGE_CURRENT_CHANGED_DISTANCE, PAGE_FINGERPRINT_STABLE_DISTANCE,
        PAGE_SUFFIX_UNCHANGED_DISTANCE,
    };

    #[test]
    fn reads_page_number_fixtures() {
        let reader = PageNumberReader::default();
        for current in 1..=20 {
            let path = format!("page_{current:02}_of_49.png");
            let image = fixture_image(&path);
            let (page, diagnostics) = reader.read_page_number_with_constraint_diagnostics(
                &image,
                PageReadConstraint::default(),
            );
            let page = page.unwrap_or_else(|error| panic!("{path}: {error}; {diagnostics:?}"));
            assert_eq!(page.current, current, "{path}");
            assert_eq!(page.total, 49, "{path}");
        }
    }

    #[test]
    fn reads_previously_difficult_fixtures_without_navigation_hint() {
        let reader = PageNumberReader::default();
        for (path, current, total) in [
            ("failure_01_of_27.png", 1, 27),
            ("failure_01_of_47.png", 1, 47),
            ("page_02_of_14.png", 2, 14),
            ("page_44_of_47.png", 44, 47),
            ("page_55_of_191.png", 55, 191),
            ("page_200_of_265_composite.png", 200, 265),
            ("page_9999_of_9999.png", 9999, 9999),
        ] {
            let image = fixture_image(path);
            let page = reader
                .read_page_number(&image)
                .unwrap_or_else(|error| panic!("{path}: {error}"));
            assert_eq!((page.current, page.total), (current, total), "{path}");
        }
    }

    #[test]
    fn known_total_does_not_change_current_ranking() {
        let reader = PageNumberReader::default();
        for path in ["page_55_of_191.png", "page_55_of_290.png"] {
            let image = fixture_image(path);
            let (plain, _) = reader
                .observe_page_number(&image, PageReadConstraint::default())
                .unwrap();
            for known_total in [191, 290, 9999] {
                let (constrained, _) = reader
                    .observe_page_number(
                        &image,
                        PageReadConstraint {
                            known_total: Some(known_total),
                        },
                    )
                    .unwrap();
                assert_eq!(plain.current, constrained.current, "{path}/{known_total}");
            }
            assert_eq!(plain.current.confident_value(), Some(55), "{path}");
        }
    }

    #[test]
    fn blank_page_number_returns_missing_observation() {
        let reader = PageNumberReader::default();
        let image = RgbaImage::new(95, 60);
        let (observation, diagnostics) = reader
            .observe_page_number(&image, PageReadConstraint::default())
            .unwrap();

        assert_eq!(observation.slash, ObservationState::Missing);
        assert_eq!(observation.current.state, ObservationState::Missing);
        assert_eq!(observation.total.state, ObservationState::Missing);
        assert!(observation.fingerprint.is_none());
        assert!(!diagnostics.error.is_empty());
    }

    #[test]
    fn mask_distance_is_brightness_invariant_and_alignment_tolerant() {
        assert!(mask_distance(&[0, 32, 64, 96], &[64, 96, 128, 160]) < 1e-6);
        assert!(mask_distance(&[0, 0, 255, 255], &[0, 255, 0, 255]) > 0.4);
        assert_eq!(mask_distance(&[], &[]), 1.0);
        assert_eq!(mask_distance(&[0], &[0, 1]), 1.0);

        let mut left = vec![0; (FINGERPRINT_WIDTH * FINGERPRINT_HEIGHT) as usize];
        let mut right = left.clone();
        for y in 8..24 {
            for x in 20..30 {
                left[(y * FINGERPRINT_WIDTH + x) as usize] = 255;
                right[(y * FINGERPRINT_WIDTH + x + 1) as usize] = 255;
            }
        }
        assert!(mask_distance(&left, &right) < 1e-6);
    }

    #[test]
    fn fingerprint_resists_global_brightness_changes() {
        let reader = PageNumberReader::default();
        let image = fixture_image("page_55_of_191.png");
        let (baseline, _) = reader
            .observe_page_number(&image, PageReadConstraint::default())
            .unwrap();
        let baseline = baseline.fingerprint.unwrap();

        for delta in [-12, -8, -4, 4, 8, 12] {
            let variant = image::imageops::brighten(&image, delta);
            let (observation, _) = reader
                .observe_page_number(&variant, PageReadConstraint::default())
                .unwrap();
            let fingerprint = observation.fingerprint.unwrap();
            assert!(
                baseline.current_distance(&fingerprint)
                    <= PAGE_FINGERPRINT_STABLE_DISTANCE,
                "current changed at brightness {delta}"
            );
            assert!(
                baseline.suffix_distance(&fingerprint)
                    <= PAGE_FINGERPRINT_STABLE_DISTANCE,
                "suffix changed at brightness {delta}"
            );
        }
    }

    #[test]
    fn fingerprint_distinguishes_adjacent_pages_and_keeps_suffix() {
        let reader = PageNumberReader::default();
        let page_54 = fixture_image("page_54_of_290.png");
        let page_55 = fixture_image("page_55_of_290.png");
        let (left, _) = reader
            .observe_page_number(&page_54, PageReadConstraint::default())
            .unwrap();
        let (right, _) = reader
            .observe_page_number(&page_55, PageReadConstraint::default())
            .unwrap();
        let left = left.fingerprint.unwrap();
        let right = right.fingerprint.unwrap();

        assert!(left.current_distance(&right) >= PAGE_CURRENT_CHANGED_DISTANCE);
        assert!(left.suffix_distance(&right) <= PAGE_SUFFIX_UNCHANGED_DISTANCE);
    }

    #[test]
    fn fingerprint_distinguishes_changed_total_suffix() {
        let reader = PageNumberReader::default();
        let page_191 = fixture_image("page_55_of_191.png");
        let page_290 = fixture_image("page_55_of_290.png");
        let (left, _) = reader
            .observe_page_number(&page_191, PageReadConstraint::default())
            .unwrap();
        let (right, _) = reader
            .observe_page_number(&page_290, PageReadConstraint::default())
            .unwrap();
        let distance = left
            .fingerprint
            .unwrap()
            .suffix_distance(&right.fingerprint.unwrap());

        assert!(distance > PAGE_SUFFIX_UNCHANGED_DISTANCE);
    }

    #[test]
    fn numeric_candidate_enumeration_is_complete_and_deterministic() {
        let slot_scores = vec![
            vec![
                GlyphCandidate {
                    ch: '1',
                    score: 0.9,
                },
                GlyphCandidate {
                    ch: '0',
                    score: 0.8,
                },
            ],
            vec![
                GlyphCandidate {
                    ch: '2',
                    score: 0.85,
                },
                GlyphCandidate {
                    ch: '3',
                    score: 0.75,
                },
            ],
        ];

        let first = number_candidates(&slot_scores);
        let second = number_candidates(&slot_scores);

        assert_eq!(
            first.iter().map(|candidate| candidate.value).collect::<Vec<_>>(),
            [12, 13]
        );
        assert_eq!(
            first
                .iter()
                .map(|candidate| candidate.score.to_bits())
                .collect::<Vec<_>>(),
            second
                .iter()
                .map(|candidate| candidate.score.to_bits())
                .collect::<Vec<_>>()
        );
        let expected_margin_score = ((0.1_f32 + 0.20) / 0.45).clamp(0.0, 1.0);
        let expected_score = 0.875 * 0.76 + 0.85 * 0.18 + expected_margin_score * 0.06;
        assert!((first[0].score - expected_score).abs() < f32::EPSILON);

        let mut by_value = HashMap::new();
        merge_field_candidates(
            &mut by_value,
            vec![
                FieldCandidate {
                    value: 12,
                    score: 0.4,
                },
                FieldCandidate {
                    value: 12,
                    score: 0.8,
                },
            ],
        );
        assert_eq!(by_value[&12].score, 0.8);
    }

    #[test]
    fn zero_sized_image_is_a_program_error() {
        let reader = PageNumberReader::default();

        let error = reader
            .observe_page_number(&RgbaImage::new(0, 0), PageReadConstraint::default())
            .unwrap_err();

        assert!(error.to_string().contains("image is empty"));
    }

    fn fixture_image(path: &str) -> RgbaImage {
        image::load_from_memory(fixture_bytes(path))
            .unwrap_or_else(|error| panic!("{path}: {error}"))
            .to_rgba8()
    }

    fn fixture_bytes(path: &str) -> &'static [u8] {
        match path {
            "page_01_of_49.png" => include_bytes!("../../tests/fixtures/page_numbers/page_01_of_49.png"),
            "page_02_of_49.png" => include_bytes!("../../tests/fixtures/page_numbers/page_02_of_49.png"),
            "page_03_of_49.png" => include_bytes!("../../tests/fixtures/page_numbers/page_03_of_49.png"),
            "page_04_of_49.png" => include_bytes!("../../tests/fixtures/page_numbers/page_04_of_49.png"),
            "page_05_of_49.png" => include_bytes!("../../tests/fixtures/page_numbers/page_05_of_49.png"),
            "page_06_of_49.png" => include_bytes!("../../tests/fixtures/page_numbers/page_06_of_49.png"),
            "page_07_of_49.png" => include_bytes!("../../tests/fixtures/page_numbers/page_07_of_49.png"),
            "page_08_of_49.png" => include_bytes!("../../tests/fixtures/page_numbers/page_08_of_49.png"),
            "page_09_of_49.png" => include_bytes!("../../tests/fixtures/page_numbers/page_09_of_49.png"),
            "page_10_of_49.png" => include_bytes!("../../tests/fixtures/page_numbers/page_10_of_49.png"),
            "page_11_of_49.png" => include_bytes!("../../tests/fixtures/page_numbers/page_11_of_49.png"),
            "page_12_of_49.png" => include_bytes!("../../tests/fixtures/page_numbers/page_12_of_49.png"),
            "page_13_of_49.png" => include_bytes!("../../tests/fixtures/page_numbers/page_13_of_49.png"),
            "page_14_of_49.png" => include_bytes!("../../tests/fixtures/page_numbers/page_14_of_49.png"),
            "page_15_of_49.png" => include_bytes!("../../tests/fixtures/page_numbers/page_15_of_49.png"),
            "page_16_of_49.png" => include_bytes!("../../tests/fixtures/page_numbers/page_16_of_49.png"),
            "page_17_of_49.png" => include_bytes!("../../tests/fixtures/page_numbers/page_17_of_49.png"),
            "page_18_of_49.png" => include_bytes!("../../tests/fixtures/page_numbers/page_18_of_49.png"),
            "page_19_of_49.png" => include_bytes!("../../tests/fixtures/page_numbers/page_19_of_49.png"),
            "page_20_of_49.png" => include_bytes!("../../tests/fixtures/page_numbers/page_20_of_49.png"),
            "failure_01_of_27.png" => include_bytes!("../../tests/fixtures/page_numbers/failure_01_of_27.png"),
            "failure_01_of_47.png" => include_bytes!("../../tests/fixtures/page_numbers/failure_01_of_47.png"),
            "page_02_of_14.png" => include_bytes!("../../tests/fixtures/page_numbers/page_02_of_14.png"),
            "page_44_of_47.png" => include_bytes!("../../tests/fixtures/page_numbers/page_44_of_47.png"),
            "page_55_of_191.png" => include_bytes!("../../tests/fixtures/page_numbers/page_55_of_191.png"),
            "page_200_of_265_composite.png" => include_bytes!("../../tests/fixtures/page_numbers/page_200_of_265_composite.png"),
            "page_9999_of_9999.png" => include_bytes!("../../tests/fixtures/page_numbers/page_9999_of_9999.png"),
            "page_54_of_290.png" => include_bytes!("../../tests/fixtures/page_numbers/page_54_of_290.png"),
            "page_55_of_290.png" => include_bytes!("../../tests/fixtures/page_numbers/page_55_of_290.png"),
            _ => panic!("unknown fixture: {path}"),
        }
    }
}
