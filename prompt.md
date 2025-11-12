rtest is rust binary to replace the python command "pytest --collect-only"
It works really well, but it does not have the same output when the python 
test file has parameterized tests.

Fix rtest to have the same output as `pytest --collect-only` when there are parameterized tests.
Refer to the @README.md file in this directory for more information.

