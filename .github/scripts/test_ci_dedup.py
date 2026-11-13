import unittest
from unittest.mock import patch

import ci_dedup


class DedupTests(unittest.TestCase):
    def setUp(self):
        self.env = {'GITHUB_EVENT_NAME': 'push', 'GITHUB_REF': 'refs/heads/master',
                    'GITHUB_SHA': 'a' * 40, 'GITHUB_REPOSITORY': 'owner/repo'}
        self.tree = 'b' * 40
        self.prs = [{'number': 7, 'merged_at': 'now', 'merge_commit_sha': self.env['GITHUB_SHA'],
                     'base': {'repo': {'full_name': 'owner/repo'}},
                     'head': {'sha': 'e' * 40, 'repo': {'full_name': 'owner/repo'}}}]
        self.run = {'id': 10, 'event': 'pull_request', 'status': 'completed', 'conclusion': 'success',
                    'repository': {'full_name': 'owner/repo'}, 'head_repository': {'full_name': 'owner/repo'},
                    'head_sha': 'e' * 40, 'pull_requests': []}
        self.jobs = [{'name': name, 'status': 'completed', 'conclusion': 'success'}
                     for name in ('verified-tree-' + self.tree, 'browser', 'arch')]

    def check(self):
        def api(path):
            if '/commits/' in path:
                return self.prs
            if '/runs/10/jobs' in path:
                return {'jobs': self.jobs}
            self.assertIn('head_sha=' + 'e' * 40, path)
            return {'workflow_runs': [self.run]}
        return ci_dedup.should_skip(self.env, self.tree, api)

    def test_successful_matching_pr_tree_skips(self):
        self.assertTrue(self.check())

    def test_missing_or_different_evidence_runs(self):
        for change in ('direct', 'tree', 'failure', 'foreign', 'missing_marker', 'non_master', 'wrong_merge', 'failed_marker', 'failed_browser', 'missing_arch', 'wrong_head', 'foreign_head'):
            with self.subTest(change=change):
                self.setUp()
                if change == 'direct': self.prs = []
                if change == 'tree': self.jobs[0]['name'] = 'verified-tree-' + 'c' * 40
                if change == 'failure': self.run['conclusion'] = 'failure'
                if change == 'foreign': self.run['repository']['full_name'] = 'other/repo'
                if change == 'missing_marker': self.jobs = []
                if change == 'non_master': self.env['GITHUB_REF'] = 'refs/heads/feature'
                if change == 'wrong_merge': self.prs[0]['merge_commit_sha'] = 'd' * 40
                if change == 'failed_marker': self.jobs[0]['conclusion'] = 'failure'
                if change == 'failed_browser': self.jobs[1]['conclusion'] = 'failure'
                if change == 'missing_arch': self.jobs.pop()
                if change == 'wrong_head': self.run['head_sha'] = 'f' * 40
                if change == 'foreign_head': self.prs[0]['head']['repo']['full_name'] = 'other/repo'
                self.assertFalse(self.check())

    def test_pull_request_does_not_query_api(self):
        self.env['GITHUB_EVENT_NAME'] = 'pull_request'
        with patch.object(ci_dedup, 'api_get', side_effect=AssertionError('unexpected network')):
            self.assertFalse(ci_dedup.safe_should_skip(self.env, self.tree))

    def test_unavailable_api_runs(self):
        with patch.object(ci_dedup, 'api_get', side_effect=OSError('offline')):
            self.assertFalse(ci_dedup.safe_should_skip(self.env, self.tree))


if __name__ == '__main__':
    unittest.main()
