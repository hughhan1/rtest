"""Unit tests for rtest skip functionality."""

from __future__ import annotations

from rtest.mark import ParametrizeSpec, mark


class TestMarkSkip:
    """Tests for the @mark.skip decorator."""

    def test_skip_sets_attributes(self) -> None:
        """@skip sets __rtest_skip__ and __rtest_skip_reason__."""

        @mark.skip(reason="not implemented")  # type: ignore[misc]
        def test_func() -> None:
            pass

        skip_flag: bool = getattr(test_func, "__rtest_skip__", False)
        skip_reason: str = getattr(test_func, "__rtest_skip_reason__", "")
        assert skip_flag is True
        assert skip_reason == "not implemented"

    def test_skip_without_reason(self) -> None:
        """@skip without reason sets empty string."""

        @mark.skip()  # type: ignore[misc]
        def test_func() -> None:
            pass

        skip_flag: bool = getattr(test_func, "__rtest_skip__", False)
        skip_reason: str = getattr(test_func, "__rtest_skip_reason__", "")
        assert skip_flag is True
        assert skip_reason == ""

    def test_skip_on_class(self) -> None:
        """@skip on class sets attributes on the class."""

        @mark.skip(reason="class skip")  # type: ignore[misc]
        class TestClass:
            def test_method(self) -> None:
                pass

        skip_flag: bool = getattr(TestClass, "__rtest_skip__", False)
        skip_reason: str = getattr(TestClass, "__rtest_skip_reason__", "")
        assert skip_flag is True
        assert skip_reason == "class skip"


class TestSkipAndParametrizeCombination:
    """Tests for combining @skip and @parametrize."""

    def test_skip_parametrized_function(self) -> None:
        """@skip and @parametrize can be combined."""

        @mark.skip(reason="WIP")  # type: ignore[misc]
        @mark.cases("x", [1, 2, 3])  # type: ignore[misc]
        def test_func(_x: int) -> None:
            pass

        # Both attributes should be present
        skip_flag: bool = getattr(test_func, "__rtest_skip__", False)
        skip_reason: str = getattr(test_func, "__rtest_skip_reason__", "")
        cases_list: list[ParametrizeSpec] = getattr(test_func, "__rtest_cases__", [])
        assert skip_flag is True
        assert skip_reason == "WIP"
        assert len(cases_list) == 1
