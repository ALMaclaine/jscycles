"""Publish ordering and rate-limit retries for scripts/publish_crates.py."""
import json
import subprocess
import unittest
from pathlib import Path
from types import SimpleNamespace

import publish_crates

ROOT = Path(__file__).resolve().parents[1]


def package(name, dependencies=(), publish=None, dev=()):
    return {'id': name, 'name': name, 'version': '1.0.0', 'publish': publish,
            'dependencies': [{'name': d, 'kind': None} for d in dependencies]
            + [{'name': d, 'kind': 'dev'} for d in dev]}


def metadata(*packages):
    return {'packages': list(packages), 'workspace_members': [p['id'] for p in packages]}


class PublishOrderTests(unittest.TestCase):
    def test_dependencies_come_first_and_unpublishable_members_are_left_out(self):
        order = publish_crates.publish_order(metadata(
            package('app', ['scan']), package('scan', ['budget', 'serde']),
            package('budget'), package('xtask', publish=[])))
        self.assertEqual([name for name, _ in order], ['budget', 'scan', 'app'])

    def test_dev_dependencies_do_not_order_publication(self):
        order = publish_crates.publish_order(metadata(package('a', dev=['b']), package('b', ['a'])))
        self.assertEqual([name for name, _ in order], ['a', 'b'])

    def test_a_dependency_cycle_is_rejected(self):
        with self.assertRaises(ValueError):
            publish_crates.publish_order(metadata(package('a', ['b']), package('b', ['a'])))

    def test_the_workspace_publishes_every_crate_after_its_dependencies(self):
        workspace = json.loads(subprocess.check_output(
            ['cargo', 'metadata', '--locked', '--no-deps', '--format-version', '1'], cwd=ROOT, text=True))
        order = [name for name, _ in publish_crates.publish_order(workspace)]
        self.assertNotIn('xtask', order)
        self.assertEqual(order[-1], 'jscycles', 'the command is published last')
        position = {name: index for index, name in enumerate(order)}
        for member in workspace['packages']:
            for dependency in member['dependencies']:
                if member['name'] in position and dependency['name'] in position and dependency.get('kind') != 'dev':
                    self.assertLess(position[dependency['name']], position[member['name']],
                                    f"{dependency['name']} before {member['name']}")


class PublishRetryTests(unittest.TestCase):
    def test_rate_limits_are_retried_and_other_failures_are_not(self):
        outcomes = [SimpleNamespace(returncode=101, stdout='', stderr='status 429 Too Many Requests'),
                    SimpleNamespace(returncode=0, stdout='', stderr='')]
        waits = []
        publish_crates.publish('a', False, run=lambda *_a, **_k: outcomes.pop(0), sleep=waits.append)
        self.assertEqual(waits, [publish_crates.RATE_LIMIT_WAIT_SECONDS])
        failure = SimpleNamespace(returncode=101, stdout='', stderr='error: invalid manifest')
        with self.assertRaises(SystemExit):
            publish_crates.publish('a', False, run=lambda *_a, **_k: failure, sleep=waits.append)
        self.assertEqual(len(waits), 1, 'an ordinary failure is not retried')


if __name__ == '__main__':
    unittest.main()
