#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_page_number_fixtures() {
        let reader = PageNumberReader::default();
        for current in 1..=20 {
            let path = format!("../tests/fixtures/page_numbers/page_{current:02}_of_49.png");
            let image = image::load_from_memory(fixture_bytes(&path))
                .unwrap_or_else(|error| panic!("{path}: {error}"))
                .to_rgba8();
            let page = reader.read_page_number(&image).unwrap_or_else(|error| {
                panic!("{path}: {error}");
            });
            assert_eq!(page.current, current, "{path}");
            assert_eq!(page.total, 49, "{path}");
        }
    }

    #[test]
    fn reads_failure_fixtures_without_windows_ocr() {
        let reader = PageNumberReader::default();
        for (path, current, total) in [
            ("../tests/fixtures/page_numbers/failure_01_of_27.png", 1, 27),
            ("../tests/fixtures/page_numbers/failure_01_of_47.png", 1, 47),
        ] {
            let image = image::load_from_memory(fixture_bytes(path))
                .unwrap_or_else(|error| panic!("{path}: {error}"))
                .to_rgba8();
            let page = reader.read_page_number(&image).unwrap_or_else(|error| {
                panic!("{path}: {error}");
            });
            assert_eq!(page.current, current, "{path}");
            assert_eq!(page.total, total, "{path}");
        }
    }

    #[test]
    fn reads_merged_page_number_with_hint() {
        let reader = PageNumberReader::default();
        let image = image::load_from_memory(fixture_bytes(
            "../tests/fixtures/page_numbers/page_44_of_47.png",
        ))
        .unwrap()
        .to_rgba8();
        let page = reader
            .read_page_number_with_hint(
                &image,
                PageReadHint {
                    previous_current: Some(43),
                    expected_current: Some(44),
                    expected_total: Some(47),
                },
            )
            .unwrap();

        assert_eq!(page.current, 44);
        assert_eq!(page.total, 47);
        assert_ne!(page.text, "8/47");
    }

    #[test]
    fn reads_small_ambiguous_page_number_with_hint() {
        let reader = PageNumberReader::default();
        let image = image::load_from_memory(fixture_bytes(
            "../tests/fixtures/page_numbers/page_02_of_14.png",
        ))
        .unwrap()
        .to_rgba8();
        let page = reader
            .read_page_number_with_hint(
                &image,
                PageReadHint {
                    previous_current: Some(1),
                    expected_current: Some(2),
                    expected_total: Some(14),
                },
            )
            .unwrap();

        assert_eq!(page.current, 2);
        assert_eq!(page.total, 14);
        assert_ne!(page.text, "7/14");
    }

    #[test]
    fn reads_four_digit_page_number_with_hint() {
        let reader = PageNumberReader::default();
        let image = fixture_image("../tests/fixtures/page_numbers/page_9999_of_9999.png");
        let text_width = measured_text_width(&image);
        assert!(text_width <= 140, "text width {text_width} exceeds page rect");

        let page = reader
            .read_page_number_with_hint(
                &image,
                PageReadHint {
                    previous_current: Some(9998),
                    expected_current: Some(9999),
                    expected_total: Some(9999),
                },
            )
            .unwrap();

        assert_eq!(page.current, 9999);
        assert_eq!(page.total, 9999);
        assert_eq!(page.text, "9999/9999");
    }

    #[test]
    fn reads_composite_200_page_number_with_hint() {
        let reader = PageNumberReader::default();
        let image = fixture_image("../tests/fixtures/page_numbers/page_200_of_265_composite.png");
        let page = reader
            .read_page_number_with_hint(
                &image,
                PageReadHint {
                    previous_current: Some(199),
                    expected_current: Some(200),
                    expected_total: Some(265),
                },
            )
            .unwrap();

        assert_eq!(page.current, 200);
        assert_eq!(page.total, 265);
        assert_eq!(page.text, "200/265");
    }

    #[test]
    fn unhinted_four_digit_page_number_fails_fast() {
        let reader = PageNumberReader::default();
        let image = fixture_image("../tests/fixtures/page_numbers/page_9999_of_9999.png");
        let (result, diagnostics) =
            reader.read_page_number_with_hint_diagnostics(&image, PageReadHint::default());

        assert!(result.is_err());
        assert!(
            diagnostics.error.contains("requires page hint"),
            "{}",
            diagnostics.error
        );
    }

    #[test]
    fn hint_does_not_fill_missing_current_page() {
        let reader = PageNumberReader::default();
        let image = image::load_from_memory(fixture_bytes(
            "../tests/fixtures/page_numbers/failure_01_of_27.png",
        ))
        .unwrap()
        .to_rgba8();
        let cropped =
            image::imageops::crop_imm(&image, 31, 0, image.width() - 31, image.height()).to_image();
        let error = reader
            .read_page_number_with_hint(
                &cropped,
                PageReadHint {
                    previous_current: None,
                    expected_current: Some(1),
                    expected_total: Some(27),
                },
            )
            .unwrap_err();
        assert!(error.to_string().contains("page number"));
    }

    #[test]
    fn blank_page_number_fails_with_diagnostics() {
        let reader = PageNumberReader::default();
        let image = RgbaImage::new(95, 60);
        let (result, diagnostics) =
            reader.read_page_number_with_hint_diagnostics(&image, PageReadHint::default());
        assert!(result.is_err());
        assert!(!diagnostics.attempts.is_empty());
        assert!(
            diagnostics
                .attempts
                .iter()
                .all(|attempt| attempt.text.is_none() && attempt.error.is_some())
        );
    }

    fn fixture_image(path: &str) -> RgbaImage {
        image::load_from_memory(fixture_bytes(path))
            .unwrap_or_else(|error| panic!("{path}: {error}"))
            .to_rgba8()
    }

    fn measured_text_width(image: &RgbaImage) -> u32 {
        THRESHOLDS
            .iter()
            .filter_map(|threshold| {
                target_from_mask(&threshold_white_text(image, *threshold), image, *threshold)
                    .map(|target| target.width)
            })
            .max()
            .expect("fixture should contain page text")
    }

    fn fixture_bytes(path: &str) -> &'static [u8] {
        match path {
            "../tests/fixtures/page_numbers/page_01_of_49.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/page_01_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_02_of_49.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/page_02_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_03_of_49.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/page_03_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_04_of_49.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/page_04_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_05_of_49.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/page_05_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_06_of_49.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/page_06_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_07_of_49.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/page_07_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_08_of_49.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/page_08_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_09_of_49.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/page_09_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_10_of_49.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/page_10_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_11_of_49.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/page_11_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_12_of_49.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/page_12_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_13_of_49.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/page_13_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_14_of_49.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/page_14_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_15_of_49.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/page_15_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_16_of_49.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/page_16_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_17_of_49.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/page_17_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_18_of_49.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/page_18_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_19_of_49.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/page_19_of_49.png")
            }
            "../tests/fixtures/page_numbers/page_20_of_49.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/page_20_of_49.png")
            }
            "../tests/fixtures/page_numbers/failure_01_of_27.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/failure_01_of_27.png")
            }
            "../tests/fixtures/page_numbers/failure_01_of_47.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/failure_01_of_47.png")
            }
            "../tests/fixtures/page_numbers/page_44_of_47.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/page_44_of_47.png")
            }
            "../tests/fixtures/page_numbers/page_02_of_14.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/page_02_of_14.png")
            }
            "../tests/fixtures/page_numbers/page_9999_of_9999.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/page_9999_of_9999.png")
            }
            "../tests/fixtures/page_numbers/page_200_of_265_composite.png" => {
                include_bytes!("../../tests/fixtures/page_numbers/page_200_of_265_composite.png")
            }
            _ => panic!("unknown fixture: {path}"),
        }
    }
}
