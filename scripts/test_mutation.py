"""A broken baseline, survivor or unrelated timeout must fail the mutation gate."""

import copy
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from mutation import (campaign_lock, capture_campaign, input_snapshot, review_results, retry_directory,
                      retry_selection, selection_digest, validate_snapshot, project_toolchain)


class MutationGateTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        (self.root / "source.rs").write_text("original")
        (self.root / "change.diff").write_text("known nonadvancing loop")
        self.policy = {
            "selected_mutants": 1,
            "selection_sha256": selection_digest(["loop"]),
            "inputs": {"source.rs": hashlib.sha256(b"original").hexdigest()},
            "timeouts": {hashlib.sha256(b"known nonadvancing loop").hexdigest(): "reviewed hang"},
        }
        self.results = [
            {"scenario": "Baseline", "summary": "Success"},
            {"scenario": {"Mutant": {"name": "loop"}}, "summary": "Timeout", "diff_path": "change.diff",
             "phase_results": [{"phase": "Build", "process_status": "Success"},
                               {"phase": "Test", "process_status": "Timeout"}]},
        ]

    def review(self, results=None, policy=None):
        (self.root / "outcomes.json").write_text(json.dumps({"outcomes": self.results if results is None else results}))
        return review_results(self.root, self.root, self.policy if policy is None else policy)

    def selection(self, candidates):
        item = copy.deepcopy(self.results[1])
        item["scenario"]["Mutant"].update(file="source.rs", span={"start": {"line": 1}})
        with patch("mutation.subprocess.check_output", return_value=json.dumps(candidates)):
            return retry_selection(self.root, self.root, item)

    def test_exact_retry_selection_is_accepted(self):
        selection, destination = self.selection([{"name": "loop", "diff": "known nonadvancing loop"}])
        self.assertIn("--in-diff", selection)
        self.assertTrue((destination / "selection.json").exists())

    def test_extra_struct_field_mutation_is_rejected_before_execution(self):
        with self.assertRaisesRegex(ValueError, "exactly one"):
            self.selection([{"name": "loop", "diff": "known nonadvancing loop"},
                            {"name": "unrelated field deletion", "diff": "different"}])

    def test_different_retry_identity_is_rejected_before_execution(self):
        with self.assertRaisesRegex(ValueError, "exactly one"):
            self.selection([{"name": "different mutation", "diff": "known nonadvancing loop"}])

    def test_different_retry_diff_is_rejected_before_execution(self):
        with self.assertRaisesRegex(ValueError, "different mutation diff"):
            self.selection([{"name": "loop", "diff": "different"}])

    def capture(self, names, effect=None):
        (self.root / "src").mkdir(exist_ok=True)
        (self.root / "src/new.rs").write_text("new implementation")
        def listed(*args, **kwargs):
            if effect:
                effect()
            return json.dumps([{"name": name} for name in names])
        with patch("mutation.subprocess.check_output", side_effect=listed):
            return capture_campaign(self.root, self.policy, ["src/**"])

    def test_fresh_campaign_uses_current_selection_and_inputs(self):
        self.policy["inputs"]["source.rs"] = "obsolete review"
        result = self.capture(["new-case-a", "new-case-b"])
        self.assertEqual(result["selected_mutants"], 2)
        self.assertEqual(result["selection_sha256"], selection_digest(["new-case-a", "new-case-b"]))
        self.assertEqual(result["inputs"]["src/new.rs"], hashlib.sha256(b"new implementation").hexdigest())

    def test_campaign_selects_project_compiler_over_inherited_shell_override(self):
        (self.root / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "1.98.1"\n')
        def version(command, **kwargs):
            self.assertEqual(os.environ["RUSTUP_TOOLCHAIN"], "1.98.1")
            return "cargo 1.98.1 (selected)" if command[0] == "cargo" else "rustc 1.98.1 (selected)\nhost: test-host"
        with patch.dict(os.environ, {"RUSTUP_TOOLCHAIN": "unavailable-inherited-compiler"}):
            with patch("mutation.subprocess.check_output", side_effect=version):
                with project_toolchain(self.root) as execution:
                    self.assertEqual(execution["toolchain"], "1.98.1")
                    self.policy["execution"] = execution
                    captured = self.capture(["case"])
                    self.assertEqual(captured["schema"], 3)
                    self.assertEqual(captured["execution"], execution)
            self.assertEqual(os.environ["RUSTUP_TOOLCHAIN"], "unavailable-inherited-compiler")

    def test_incorrect_actual_compiler_fails_before_mutations_and_restores_environment(self):
        (self.root / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "1.98.1"\n')
        with patch.dict(os.environ, {"RUSTUP_TOOLCHAIN": "caller-compiler"}):
            with patch("mutation.subprocess.check_output", return_value="rustc 1.96.1 (wrong)"):
                with self.assertRaisesRegex(ValueError, "compiler identity"):
                    with project_toolchain(self.root):
                        self.fail("must reject before entering the campaign")
            self.assertEqual(os.environ["RUSTUP_TOOLCHAIN"], "caller-compiler")

    def test_new_manifest_cannot_omit_or_mislabel_actual_compiler(self):
        for execution in [None, {"toolchain": "1.98.1", "cargo_version": "cargo 1.96.1 (wrong)",
                                 "rustc_verbose": "rustc 1.98.1 (selected)"}]:
            with self.subTest(execution=execution):
                policy = {**self.policy, "schema": 3, "execution": execution}
                with self.assertRaisesRegex(ValueError, "compiler"):
                    self.review(policy=policy)

    def test_empty_and_duplicate_selections_are_rejected(self):
        for names in [[], ["same", "same"]]:
            with self.subTest(names=names), self.assertRaisesRegex(ValueError, "unique case identities"):
                self.capture(names)

    def test_source_edits_during_selection_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "changed during selection"):
            self.capture(["case"], lambda: (self.root / "src/new.rs").write_text("changed"))

    def test_new_source_file_invalidates_existing_campaign(self):
        campaign = self.capture(["case"])
        (self.root / "src/added.rs").write_text("new module")
        with self.assertRaisesRegex(ValueError, "stale"):
            validate_snapshot(self.root, campaign["inputs"])

    def test_deleted_source_invalidates_existing_campaign(self):
        campaign = self.capture(["case"])
        (self.root / "src/new.rs").unlink()
        with self.assertRaisesRegex(ValueError, "stale"):
            validate_snapshot(self.root, campaign["inputs"])

    def test_policy_edits_invalidate_existing_campaign(self):
        (self.root / ".cargo").mkdir()
        policy = self.root / ".cargo/mutation-review.json"
        policy.write_text('{"files":["src/**"]}')
        before = input_snapshot(self.root)
        policy.write_text('{"files":["src/only.rs"]}')
        with self.assertRaisesRegex(ValueError, "stale"):
            validate_snapshot(self.root, before)

    def test_generated_cargo_home_does_not_change_checkout_snapshot(self):
        before = input_snapshot(self.root)
        registry = self.root / ".cargo/registry/src/package"
        registry.mkdir(parents=True)
        (registry / "Cargo.toml").write_text("generated dependency")
        self.assertEqual(input_snapshot(self.root), before)
        (self.root / ".cargo/config.toml").write_text("[build]")
        self.assertNotEqual(input_snapshot(self.root), before)

    def test_overlapping_controllers_are_rejected_before_writing_metadata(self):
        with campaign_lock(self.root):
            with self.assertRaisesRegex(ValueError, "already owns"):
                with campaign_lock(self.root):
                    self.fail("must not enter a second controller")
        with campaign_lock(self.root):
            self.assertTrue((self.root / "runner.lock").exists())

    def test_failed_controller_releases_its_lock(self):
        with self.assertRaisesRegex(ValueError, "injected"):
            with campaign_lock(self.root):
                raise ValueError("injected failure")
        self.assertFalse((self.root / "runner.lock").exists())

    def test_results_from_another_tool_version_are_rejected(self):
        policy = copy.deepcopy(self.policy)
        policy["tool"] = "cargo-mutants 27.1.0"
        with self.assertRaisesRegex(ValueError, "different tool version"):
            self.review(policy=policy)

    def test_exact_reviewed_hang_is_separate_from_caught(self):
        self.assertEqual(self.review(), {"caught": 0, "unviable": 0, "reviewed_timeouts": 1})

    def test_stale_source_is_rejected(self):
        (self.root / "source.rs").write_text("changed")
        with self.assertRaisesRegex(ValueError, "stale"):
            self.review()

    def test_unknown_timeout_is_rejected(self):
        (self.root / "change.diff").write_text("different hang")
        with self.assertRaisesRegex(ValueError, "unreviewed timeout"):
            self.review()

    def make_retry(self, status="CaughtMutant", baseline="Success", changed_diff=False):
        self.policy["timeouts"] = {}
        replay = retry_directory(self.root, self.results[1])
        replay.mkdir(parents=True)
        (replay / "change.diff").write_text("different" if changed_diff else "known nonadvancing loop")
        result = copy.deepcopy(self.results[1])
        result["summary"] = status
        result["phase_results"][1]["process_status"] = {"Failure": 101}
        (replay / "outcomes.json").write_text(json.dumps({"outcomes": [
            {"scenario": "Baseline", "summary": baseline}, result,
        ]}))

    def test_exact_caught_retry_resolves_unknown_timeout(self):
        self.make_retry()
        self.assertEqual(self.review(), {"caught": 1, "unviable": 0, "reviewed_timeouts": 0, "replayed_timeouts": 1})

    def test_retry_cannot_approve_a_survivor_or_another_timeout(self):
        for status in ["MissedMutant", "Timeout", "Unviable"]:
            with self.subTest(status=status):
                replay = retry_directory(self.root, self.results[1])
                if replay.exists():
                    import shutil
                    shutil.rmtree(replay)
                self.make_retry(status=status)
                with self.assertRaisesRegex(ValueError, "exact same"):
                    self.review()

    def test_retry_requires_a_passing_baseline(self):
        self.make_retry(baseline="Failure")
        with self.assertRaisesRegex(ValueError, "passing baseline"):
            self.review()

    def test_retry_cannot_substitute_a_different_diff(self):
        self.make_retry(changed_diff=True)
        with self.assertRaisesRegex(ValueError, "exact same"):
            self.review()

    def test_caught_summary_cannot_hide_a_retry_timeout(self):
        self.make_retry()
        path = retry_directory(self.root, self.results[1]) / "outcomes.json"
        result = json.loads(path.read_text())
        result["outcomes"][1]["phase_results"][1]["process_status"] = "Timeout"
        path.write_text(json.dumps(result))
        with self.assertRaisesRegex(ValueError, "actual test failure"):
            self.review()

    def test_survivor_is_rejected(self):
        self.results[1]["summary"] = "MissedMutant"
        with self.assertRaisesRegex(ValueError, "unresolved mutation"):
            self.review()

    def test_failed_baseline_is_rejected(self):
        self.results[0]["summary"] = "Failure"
        with self.assertRaisesRegex(ValueError, "baseline"):
            self.review()

    def test_build_timeout_is_rejected(self):
        self.results[1]["phase_results"][0]["process_status"] = "Timeout"
        with self.assertRaisesRegex(ValueError, "build timeout"):
            self.review()

    def test_empty_or_partial_campaign_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "empty or incomplete"):
            self.review(self.results[:1])
        policy = copy.deepcopy(self.policy)
        policy["selected_mutants"] = 2
        with self.assertRaisesRegex(ValueError, "empty or incomplete"):
            self.review(policy=policy)

    def test_only_uncompilable_mutants_are_rejected(self):
        self.results[1]["summary"] = "Unviable"
        with self.assertRaisesRegex(ValueError, "no runtime mutation"):
            self.review()

    def test_different_mutations_with_the_same_count_are_rejected(self):
        self.results[1]["scenario"]["Mutant"]["name"] = "unreviewed replacement"
        with self.assertRaisesRegex(ValueError, "identities"):
            self.review()

    def test_duplicate_results_cannot_fill_a_missing_mutation(self):
        self.results.append(copy.deepcopy(self.results[1]))
        policy = copy.deepcopy(self.policy)
        policy["selected_mutants"] = 2
        policy["selection_sha256"] = selection_digest(["loop", "other"])
        with self.assertRaisesRegex(ValueError, "identities"):
            self.review(policy=policy)
