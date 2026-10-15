//! Shared imports keep their constant and inheritance behavior across collections.

use indoc::indoc;
use rtest::collect_tests_rust;
use std::fs;
use tempfile::TempDir;

#[test]
fn shared_constants_and_bases_are_reused_without_stale_collection_state() {
    let project = TempDir::new().expect("temporary project");
    let root = project.path().join("project");
    fs::create_dir(&root).expect("create visible project directory");
    fs::write(
        root.join("test_imported.py"),
        indoc! {r#"
            import pytest
            from shared import DATA, Base

            @pytest.mark.parametrize("value", DATA)
            def test_function(value): pass

            class TestOne(Base): pass
            class TestTwo(Base): pass
        "#},
    )
    .expect("write test module");
    fs::write(
        root.join("test_other.py"),
        "from shared import Base\nclass TestOther(Base): pass\n",
    )
    .expect("write second test module");

    for (values, ids) in [("[1, 2]", vec!["1", "2"]), ("[3]", vec!["3"])] {
        fs::write(
            root.join("shared.py"),
            format!(
                indoc! {r#"
                    import pytest
                    DATA = {values}

                    class Base:
                        @pytest.mark.parametrize("value", DATA)
                        def test_base(self, value): pass
                "#},
                values = values
            ),
        )
        .expect("write shared constants and base class");

        let (mut nodes, errors) = collect_tests_rust(root.to_path_buf(), &[])
            .expect("collect imported constants and inherited methods");
        assert!(errors.errors.is_empty(), "{:?}", errors.errors);
        assert!(errors.warnings.is_empty(), "{:?}", errors.warnings);

        let mut expected: Vec<String> = [
            "test_imported.py::test_function",
            "test_imported.py::TestOne::test_base",
            "test_imported.py::TestTwo::test_base",
            "test_other.py::TestOther::test_base",
        ]
        .into_iter()
        .flat_map(|node| ids.iter().map(move |id| format!("{node}[{id}]")))
        .collect();
        nodes.sort();
        expected.sort();
        assert_eq!(nodes, expected);
    }
}
