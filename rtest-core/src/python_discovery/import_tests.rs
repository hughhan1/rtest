#[cfg(test)]
mod tests {
    use crate::python_discovery::{discover_tests, TestDiscoveryConfig};
    use std::path::PathBuf;

    #[test]
    fn test_all_import_patterns() {
        let source = r#"
# Pattern 1: Simple import
import pytest

# Pattern 2: From import
from pytest import mark

# Pattern 3: Aliased import
import pytest as pt

# Pattern 4: From import with alias
from pytest import mark as m

# Pattern 5: Multiple from imports
from pytest import mark, fixture

# Pattern 6: Nested import
from pytest.mark import parametrize

# Test basic pytest.mark.xdist_group
@pytest.mark.xdist_group("group1")
def test_basic_import():
    pass

# Test from import mark
@mark.xdist_group("group2")
def test_from_import():
    pass

# Test aliased pytest
@pt.mark.xdist_group("group3")
def test_aliased_import():
    pass

# Test aliased mark
@m.xdist_group("group4")
def test_aliased_mark():
    pass

# Test direct parametrize import
@parametrize("value", [1, 2, 3])
def test_direct_parametrize():
    pass

# Test multiple decorators
@pytest.mark.skip
@pytest.mark.xdist_group(name="group5")
def test_multiple_decorators():
    pass

# Test class with xdist_group
@pytest.mark.xdist_group("class_group")
class TestClass:
    def test_method(self):
        pass
    
    @pytest.mark.xdist_group("method_group")
    def test_method_with_own_group(self):
        pass
"#;

        let config = TestDiscoveryConfig::default();
        let tests = discover_tests(&PathBuf::from("test.py"), source, &config).unwrap();

        // Check we found all tests
        assert_eq!(tests.len(), 8);

        // Check specific xdist_group values
        let test_basic = tests.iter().find(|t| t.name == "test_basic_import").unwrap();
        assert_eq!(test_basic.xdist_group, Some("group1".to_string()));

        let test_from = tests.iter().find(|t| t.name == "test_from_import").unwrap();
        assert_eq!(test_from.xdist_group, Some("group2".to_string()));

        let test_aliased = tests.iter().find(|t| t.name == "test_aliased_import").unwrap();
        assert_eq!(test_aliased.xdist_group, Some("group3".to_string()));

        let test_aliased_mark = tests.iter().find(|t| t.name == "test_aliased_mark").unwrap();
        assert_eq!(test_aliased_mark.xdist_group, Some("group4".to_string()));

        let test_parametrize = tests.iter().find(|t| t.name == "test_direct_parametrize").unwrap();
        assert_eq!(test_parametrize.xdist_group, None);

        let test_multiple = tests.iter().find(|t| t.name == "test_multiple_decorators").unwrap();
        assert_eq!(test_multiple.xdist_group, Some("group5".to_string()));

        let test_method = tests.iter().find(|t| t.name == "test_method").unwrap();
        assert_eq!(test_method.xdist_group, None); // Class decorators not supported yet

        let test_method_group = tests.iter().find(|t| t.name == "test_method_with_own_group").unwrap();
        assert_eq!(test_method_group.xdist_group, Some("method_group".to_string()));
    }
}