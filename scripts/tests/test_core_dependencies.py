"""Exercise the dependency guard against real, offline Cargo workspaces."""

import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "check-core-dependencies.py"
SPEC = importlib.util.spec_from_file_location("core_dependencies", SCRIPT)
GUARD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GUARD)


class CoreDependencyTests(unittest.TestCase):
    def check_workspace(self, dependency_kind: str, transitive: bool = False) -> None:
        with tempfile.TemporaryDirectory(prefix="cdl-dependency-guard-") as directory:
            root = Path(directory)
            names = ["cdl-model", "bridge", "arrow-forbidden"]
            (root / "Cargo.toml").write_text(
                '[workspace]\nmembers = ["cdl-model", "bridge", "arrow-forbidden"]\n'
                'resolver = "2"\n'
            )
            for name in names:
                package = root / name
                (package / "src").mkdir(parents=True)
                (package / "src/lib.rs").write_text("")
                (package / "Cargo.toml").write_text(
                    f'[package]\nname = "{name}"\nversion = "0.1.0"\nedition = "2021"\n'
                )
            dependency = "bridge" if transitive else "arrow-forbidden"
            with (root / "cdl-model/Cargo.toml").open("a") as manifest:
                manifest.write(f'\n[{dependency_kind}]\n{dependency} = {{ path = "../{dependency}" }}\n')
            if transitive:
                with (root / "bridge/Cargo.toml").open("a") as manifest:
                    manifest.write('\n[dependencies]\narrow-forbidden = { path = "../arrow-forbidden" }\n')
            subprocess.run(["cargo", "generate-lockfile", "--offline"], cwd=root, check=True,
                           capture_output=True)
            previous_root = GUARD.ROOT
            try:
                GUARD.ROOT = root
                GUARD.main()
            finally:
                GUARD.ROOT = previous_root

    def test_development_oracle_is_allowed(self) -> None:
        self.check_workspace("dev-dependencies")

    def test_transitive_dependency_is_rejected(self) -> None:
        with self.assertRaisesRegex(SystemExit, "arrow-forbidden"):
            self.check_workspace("dependencies", transitive=True)

    def test_target_specific_dependency_is_rejected(self) -> None:
        with self.assertRaisesRegex(SystemExit, "arrow-forbidden"):
            self.check_workspace("target.'cfg(target_arch = \"wasm32\")'.dependencies")


if __name__ == "__main__":
    unittest.main()
