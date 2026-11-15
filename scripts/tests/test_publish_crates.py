"""Protect resumable publication from duplicate uploads and early cooldown retries."""
import importlib.util
from pathlib import Path
import subprocess
import unittest
import urllib.error
from unittest.mock import patch, Mock

PATH = Path(__file__).resolve().parents[1] / "publish-crates.py"
SPEC = importlib.util.spec_from_file_location("publish_crates", PATH)
RELEASE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RELEASE)


class PublicationTests(unittest.TestCase):
    def test_matching_existing_version_is_not_uploaded(self):
        with patch.object(RELEASE, "published", return_value={"checksum": "matching"}), patch.object(RELEASE.subprocess, "run") as run:
            RELEASE.publish_one("cdl-model", "0.1.0")
            run.assert_not_called()

    def test_mismatched_published_source_stops_without_upload(self):
        with patch.object(RELEASE, "published", side_effect=ValueError("source differs")), patch.object(RELEASE.subprocess, "run") as run:
            with self.assertRaisesRegex(ValueError, "source differs"):
                RELEASE.publish_one("cdl-model", "0.1.0")
            run.assert_not_called()

    def test_cooldown_waits_until_registry_time_before_retry(self):
        limited = Mock(returncode=101, stdout="status 429 Too Many Requests: Please try again after Thu, 01 Jan 1970 00:01:40 GMT")
        success = Mock(returncode=0, stdout="Published")
        now = [90.0]
        def sleep(seconds):
            self.assertGreater(seconds, 0)
            self.assertLessEqual(seconds, 30)
            now[0] += seconds
        def upload(*args, **kwargs):
            if now[0] < 102:
                return limited
            return success
        with patch.object(RELEASE, "published", side_effect=[None, {"checksum": "matching"}]), \
             patch.object(RELEASE.time, "time", side_effect=lambda: now[0]), \
             patch.object(RELEASE.time, "sleep", side_effect=sleep), \
             patch.object(RELEASE.subprocess, "run", side_effect=upload) as run:
            RELEASE.publish_one("cdl-verify", "0.1.0")
            self.assertEqual(run.call_count, 2)
            self.assertGreaterEqual(now[0], 102)

    def test_hard_failure_is_not_retried(self):
        with patch.object(RELEASE, "published", return_value=None), \
             patch.object(RELEASE.subprocess, "run", return_value=Mock(returncode=101, stdout="status 400 email required")) as run:
            with self.assertRaises(subprocess.CalledProcessError):
                RELEASE.publish_one("cdl-model", "0.1.0")
            self.assertEqual(run.call_count, 1)

    def test_absent_metadata_is_the_only_404_treated_as_unpublished(self):
        missing = urllib.error.HTTPError("https://crates.io/example", 404, "missing", None, None)
        with patch.object(RELEASE.VERIFY, "fetch", side_effect=missing), \
             patch.object(RELEASE.VERIFY, "verify_archive") as archive:
            self.assertIsNone(RELEASE.published("cdl-model", "0.1.0"))
            archive.assert_not_called()
        missing.close()

    def test_missing_archive_for_existing_metadata_stops(self):
        missing = urllib.error.HTTPError("https://crates.io/example", 404, "missing archive", None, None)
        with patch.object(RELEASE.VERIFY, "fetch", return_value=b"metadata"), \
             patch.object(RELEASE.VERIFY, "verify_archive", side_effect=missing):
            with self.assertRaises(urllib.error.HTTPError):
                RELEASE.published("cdl-model", "0.1.0")
        missing.close()

    def test_check_mode_never_uploads(self):
        with patch.object(RELEASE, "published", return_value=None), patch.object(RELEASE.subprocess, "run") as run:
            RELEASE.publish_one("cdl-model", "0.1.0", check=True)
            run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
