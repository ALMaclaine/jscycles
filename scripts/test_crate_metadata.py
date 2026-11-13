"""crates.io accepts only some package metadata, and `cargo publish --dry-run`
does not check it: the server rejected the first 0.2.0 publish for a keyword
of 21 characters. These tests apply crates.io's published rules locally."""

from pathlib import Path
import re
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[1]

# crates.io: at most five keywords, each 1-20 ASCII characters, starting with
# a letter or digit and otherwise letters, digits, `_`, `-` or `+`.
KEYWORD = re.compile(r'[A-Za-z0-9][A-Za-z0-9_+-]{0,19}')


def metadata_problems(package):
    problems = []
    keywords = package.get('keywords', [])
    if len(keywords) > 5:
        problems.append(f'{len(keywords)} keywords; crates.io allows 5')
    problems += [f'invalid keyword {word!r}' for word in keywords if not KEYWORD.fullmatch(word)]
    categories = package.get('categories', [])
    if len(categories) > 5:
        problems.append(f'{len(categories)} categories; crates.io allows 5')
    problems += [f'invalid category slug {slug!r}' for slug in categories
                 if not re.fullmatch(r'[a-z0-9-]+(::[a-z0-9-]+)*', slug)]
    if not package.get('description', '').strip():
        problems.append('missing description')
    # `license.workspace = true` inherits the workspace's license expression.
    if 'license' not in package and 'license-file' not in package:
        problems.append('missing license')
    return problems


class CrateMetadataTests(unittest.TestCase):
    def test_published_metadata_meets_crates_io_rules(self):
        package = tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']
        self.assertEqual(metadata_problems(package), [])

    def test_rules_reject_what_crates_io_rejected(self):
        package = {'description': 'x', 'license': 'MIT',
                   'keywords': ['circular-dependencies'], 'categories': ['development-tools']}
        self.assertEqual(metadata_problems(package), ["invalid keyword 'circular-dependencies'"])
        package['keywords'] = ['a', 'b', 'c', 'd', 'e', 'f']
        self.assertEqual(metadata_problems(package), ['6 keywords; crates.io allows 5'])
        package['keywords'] = ['-leading', 'ok_1', 'c++']
        self.assertEqual(metadata_problems(package), ["invalid keyword '-leading'"])


if __name__ == '__main__':
    unittest.main()
