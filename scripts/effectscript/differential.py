#!/usr/bin/env python3
"""Compiles TypeScript test files with two tsgo binaries and reports any
difference in diagnostics, exit code or emitted files.

usage: scripts/effectscript/differential.py <base-tsgo> <new-tsgo> <out-dir> <file>...
"""
import concurrent.futures
import filecmp
import json
import os
import subprocess
import sys

FLAGS = ["--ignoreConfig", "--pretty", "false", "--declaration", "--target", "es2022", "--module", "esnext",
         "--skipLibCheck", "--types", ""]


def run(tsgo, source, out):
    os.makedirs(out, exist_ok=True)
    try:
        proc = subprocess.run([tsgo, os.path.abspath(source), "--outDir", os.path.abspath(out),
                               *FLAGS], capture_output=True, text=True, timeout=120, cwd=out)
        return proc.returncode, proc.stdout
    except subprocess.TimeoutExpired:
        return "timeout", ""


def outputs(out):
    found = {}
    for root, _, files in os.walk(out):
        for name in files:
            path = os.path.join(root, name)
            found[os.path.relpath(path, out)] = path
    return found


def compare(args):
    index, base, new, out_dir, source = args
    base_out = os.path.join(out_dir, str(index), "base")
    new_out = os.path.join(out_dir, str(index), "new")
    base_result = run(base, source, base_out)
    new_result = run(new, source, new_out)
    problems = []
    if base_result != new_result:
        problems.append("diagnostics or exit code")
    base_files, new_files = outputs(base_out), outputs(new_out)
    if set(base_files) != set(new_files):
        problems.append("output file names")
    else:
        for name in base_files:
            if not filecmp.cmp(base_files[name], new_files[name], shallow=False):
                problems.append(f"output {name}")
    return source, problems


def main():
    if len(sys.argv) < 5:
        print(__doc__)
        return 2
    base, new, out_dir, *sources = sys.argv[1:]
    base, new = os.path.abspath(base), os.path.abspath(new)
    jobs = [(i, base, new, out_dir, s) for i, s in enumerate(sources)]
    differences = {}
    with concurrent.futures.ThreadPoolExecutor(max_workers=os.cpu_count()) as pool:
        for source, problems in pool.map(compare, jobs):
            if problems:
                differences[source] = problems
    emitted = sum(1 for i in range(len(sources))
                  if any(n.endswith(".js") for n in outputs(os.path.join(out_dir, str(i), "new"))))
    print(json.dumps({"files": len(sources), "files_with_js_output": emitted,
                      "different": len(differences), "differences": differences}, indent=2))
    return 1 if differences else 0


if __name__ == "__main__":
    sys.exit(main())
