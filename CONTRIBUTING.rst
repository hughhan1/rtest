Contributing to rtest
====================

Thank you for your interest in contributing to rtest! This document provides guidelines for contributors.

Getting Started
---------------

1. Fork the repository on GitHub
2. Clone your fork locally::

    git clone https://github.com/yourusername/rtest.git
    cd rtest

3. Initialize git submodules (required for ruff dependency)::

    git submodule update --init --recursive

4. Install development dependencies::

    uv sync --dev

5. Set up Rust toolchain::

    rustup update stable

Development Workflow
--------------------

1. Create a feature branch::

    git checkout -b feature/your-feature-name

2. Install in development mode::

    uv run maturin develop

3. Run tests to ensure everything works::

    uv run pytest tests/
    cargo test

4. Make your changes following our coding standards

5. Run linting and type checking::

    uv run ruff format python/ tests/
    uv run ruff check python/ tests/
    uv run mypy python/
    cargo fmt
    cargo clippy

6. Commit your changes using conventional commits::

    git commit -m "feat: add new feature description"

7. Push to your fork and create a pull request

Commit Messages
--------------

We use `Conventional Commits <https://www.conventionalcommits.org/>`_ for automated versioning:

- ``feat: add new feature`` → minor version bump
- ``fix: resolve bug`` → patch version bump  
- ``feat!: breaking change`` → major version bump
- ``docs: update documentation`` → no version bump

Common types: ``feat``, ``fix``, ``docs``, ``style``, ``refactor``, ``test``, ``chore``

Testing
-------

Run the full test suite::

    # Python tests
    uv run pytest tests/ -v
    
    # Rust tests
    cargo test
    
    # Integration tests
    uv run maturin develop
    uv run python -c "import rtest; print('Import successful')"

For maintainers, see the `Release Process`_ section below.

Release Process
---------------

*This section is for maintainers only.*

Overview
~~~~~~~~

rtest uses automated semantic versioning with ``python-semantic-release``. Releases are triggered by conventional commits and published automatically to PyPI.

Setup Requirements
~~~~~~~~~~~~~~~~~~

Repository secrets (Settings → Secrets and variables → Actions):

- ``PYPI_API_TOKEN``: From `PyPI Account Settings <https://pypi.org/manage/account/token/>`_
- ``TEST_PYPI_API_TOKEN``: From `TestPyPI Account Settings <https://test.pypi.org/manage/account/token/>`_

Branch Workflow
~~~~~~~~~~~~~~~

- ``main`` branch: Production releases → PyPI

Releases
~~~~~~~~~~~~~~~~~~~

1. Merge to main::

    git checkout main
    git merge develop
    git push origin main

2. Release happens automatically:
   - Version bumped in ``pyproject.toml``
   - Changelog updated
   - Git tag created
   - GitHub release published
   - Multi-platform wheels built
   - Package published to PyPI

Troubleshooting
~~~~~~~~~~~~~~~

**"No version to release"**: Ensure commits follow conventional format

**Build failures**: Check Rust toolchain and Python environment::

    rustup update
    cargo check
    uv run maturin build --release

**Upload failures**: Check for existing versions::

    uv pip index versions rtest
    uv pip index versions --index-url https://test.pypi.org/simple/ rtest

Community
---------

- **Issues**: Report bugs and request features on GitHub
- **Discussions**: Join conversations in GitHub Discussions
- **Security**: Report security issues privately via GitHub Security tab

Code of Conduct
---------------

This project follows the `Contributor Covenant Code of Conduct <https://www.contributor-covenant.org/version/2/1/code_of_conduct/>`_. 
By participating, you agree to uphold this code.

License
-------

By contributing to rtest, you agree that your contributions will be licensed under the MIT License.