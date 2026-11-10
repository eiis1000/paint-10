import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "privacy", Path(__file__).parents[1] / "check-repository-privacy.py"
)
privacy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(privacy)


class PrivacyChecks(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = Path(self.temp.name)
        self.git("init", "-q")
        self.git("config", "user.name", "public-handle")
        self.git("config", "user.email", "public-handle@users.noreply.github.com")
        self.git("config", "commit.gpgsign", "false")
        self.git("config", "core.hooksPath", str(self.repo / "no-hooks"))

    def git(self, *args):
        return subprocess.check_output(["git", "-C", str(self.repo), *args], stderr=subprocess.DEVNULL)

    def stage(self, name, text):
        (self.repo / name).write_text(text)
        self.git("add", name)

    def test_public_identity_and_third_party_attribution_are_allowed(self):
        self.stage("LICENSE", "Third Party <author@example.org>\n")
        self.assertEqual(privacy.check(self.repo), [])

    def test_author_identity_is_not_restricted(self):
        self.git("config", "user.name", "Example Person")
        self.git("config", "user.email", "person@example.org")
        self.assertEqual(privacy.check(self.repo), [])
        self.stage("README.md", "Public project\n")
        self.git("commit", "-qm", "Add project")
        self.assertEqual(privacy.check(self.repo, True), [])

    def test_private_notes_are_rejected(self):
        self.stage("PROGRESS.md", "Private working notes\n")
        self.assertTrue(any("Private material" in error for error in privacy.check(self.repo)))

    def test_home_paths_are_rejected_without_echoing_contents(self):
        self.stage("notes.txt", "/" + "home/example-private-user/project\n")
        errors = privacy.check(self.repo)
        self.assertTrue(errors)
        self.assertNotIn("example-private-user", str(errors))

    def test_deleting_private_notes_does_not_clean_history(self):
        self.stage("PROGRESS.md", "Private working notes\n")
        self.git("commit", "-qm", "Add notes")
        self.git("rm", "PROGRESS.md")
        self.git("commit", "-qm", "Remove notes")
        self.assertEqual(privacy.check(self.repo), [])
        self.assertTrue(any("Private material" in error for error in privacy.check(self.repo, True)))


if __name__ == "__main__":
    unittest.main()
