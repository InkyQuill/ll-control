"""Fail-open post-merge CI deduplication using successful PR workflow evidence."""
import json
import os
import re
import subprocess
import urllib.request


def api_get(path):
    request = urllib.request.Request(
        os.environ.get('GITHUB_API_URL', 'https://api.github.com') + path,
        headers={'Authorization': 'Bearer ' + os.environ['GITHUB_TOKEN'],
                 'Accept': 'application/vnd.github+json',
                 'X-GitHub-Api-Version': '2022-11-28'},
    )
    with urllib.request.urlopen(request, timeout=10) as response:
        return json.load(response)


def should_skip(env, tree, api):
    if env.get('GITHUB_EVENT_NAME') != 'push' or env.get('GITHUB_REF') != 'refs/heads/master':
        return False
    sha, repo = env.get('GITHUB_SHA', ''), env.get('GITHUB_REPOSITORY', '')
    if not re.fullmatch(r'[0-9a-f]{40}', sha) or not re.fullmatch(r'[0-9a-f]{40}', tree):
        return False
    root = '/repos/' + repo
    prs = api(root + '/commits/' + sha + '/pulls?per_page=100')
    candidates = {pr['number'] for pr in prs
                  if pr.get('merged_at') and pr.get('merge_commit_sha') == sha
                  and (pr.get('base', {}).get('repo') or {}).get('full_name') == repo}
    if not candidates:
        return False
    runs = api(root + '/actions/workflows/linux.yml/runs?event=pull_request&per_page=100')['workflow_runs']
    for run in runs:
        if not (run.get('event') == 'pull_request' and run.get('status') == 'completed'
                and run.get('conclusion') == 'success'
                and (run.get('repository') or {}).get('full_name') == repo
                and (run.get('head_repository') or {}).get('full_name') == repo
                and candidates.intersection(pr.get('number') for pr in run.get('pull_requests', []))):
            continue
        jobs = api(root + '/actions/runs/' + str(run['id']) + '/jobs?per_page=100')['jobs']
        successful = {job.get('name') for job in jobs if job.get('status') == 'completed'
                      and job.get('conclusion') == 'success'}
        if {'verified-tree-' + tree, 'browser', 'arch'} <= successful:
            return True
    return False


def safe_should_skip(env, tree):
    try:
        return should_skip(env, tree, api_get)
    except Exception:  # Missing, malformed, or unavailable evidence means run CI.
        return False


def main():
    tree = subprocess.check_output(['git', 'rev-parse', 'HEAD^{tree}'], text=True).strip()
    skip = safe_should_skip(os.environ, tree)
    with open(os.environ['GITHUB_OUTPUT'], 'a', encoding='utf-8') as output:
        output.write(f'tree={tree}\nskip={str(skip).lower()}\n')
    print('Skipping duplicate heavy jobs' if skip else 'Running heavy jobs')


if __name__ == '__main__':
    main()
