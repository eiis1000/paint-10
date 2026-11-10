"""Check the index, or reachable history, for known privacy regression classes.

Run with the project's Python environment. Reports locations, never matched
content. Third-party license attribution is deliberately left intact.
"""

import argparse
from pathlib import Path
import re
import subprocess


def check(repo, history=False):
    def git(*args):
        return subprocess.check_output(["git", "-C", str(repo), *args])

    errors = []
    if history:
        paths = git("log", "--all", "--format=", "--name-only").splitlines()
        objects = git("rev-list", "--objects", "--all").splitlines()
    else:
        entries = git("ls-files", "--stage", "-z").split(b"\0")
        paths, objects = [], []
        for entry in filter(None, entries):
            meta, path = entry.split(b"\t", 1)
            paths.append(path)
            objects.append(meta.split()[1] + b" " + path)

    for path in set(paths):
        if (path in (b"PROGRESS.md", b"PARITY_AUDIT.md")
                or path.startswith((b"tmp/", b".agents/", b".codex/", b".aws/"))):
            errors.append("Private material tracked at " + path.decode(errors="replace"))

    local_path = re.compile(rb"/(?:home|Users)/[A-Za-z0-9_.-]+|[A-Z]:\\Users\\[A-Za-z0-9_.-]+")
    proc = subprocess.Popen(["git", "-C", str(repo), "cat-file", "--batch"],
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    seen = set()
    for entry in objects:
        oid, _, path = entry.partition(b" ")
        if oid in seen:
            continue
        seen.add(oid)
        proc.stdin.write(oid + b"\n")
        proc.stdin.flush()
        header = proc.stdout.readline().split()
        data = proc.stdout.read(int(header[2]))
        proc.stdout.read(1)
        if header[1] == b"blob" and b"\0" not in data and local_path.search(data):
            errors.append("Home-directory path in " + path.decode(errors="replace"))
    proc.stdin.close()
    proc.stdout.close()
    proc.wait()
    return sorted(set(errors))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--all-history", action="store_true")
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    problems = check(args.repo, args.all_history)
    for problem in problems:
        print(problem)
    if problems:
        raise SystemExit(1)
    print("Repository privacy regression checks passed.")
