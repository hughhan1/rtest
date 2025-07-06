#!/usr/bin/env python3
"""Test file to verify --dist loadgroup functionality"""

import pytest

@pytest.mark.xdist_group(name="database")
def test_database_1():
    """Test database functionality - group 1"""
    assert True

@pytest.mark.xdist_group(name="database") 
def test_database_2():
    """Test database functionality - group 2"""
    assert True

@pytest.mark.xdist_group("ui")
def test_ui_1():
    """Test UI functionality - group 1"""
    assert True

@pytest.mark.xdist_group("ui")
def test_ui_2():
    """Test UI functionality - group 2"""
    assert True

def test_ungrouped_1():
    """Test without xdist_group mark"""
    assert True

def test_ungrouped_2():
    """Another test without xdist_group mark"""
    assert True

class TestWithGroup:
    @pytest.mark.xdist_group(name="slow")
    def test_method_with_group(self):
        """Test method with group mark"""
        assert True
    
    def test_method_without_group(self):
        """Test method without group mark"""
        assert True