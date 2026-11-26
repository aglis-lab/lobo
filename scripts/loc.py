import os
import sys

SOURCE_EXTENSION = ".rs"

# Paths are relative to this script's directory (scripts/), i.e. the repo root is "..".
LOC_CHECKS = [
    {
        "name": "lobo",
        "paths": [
            "../src",
            "../examples",
            "../tests",
        ],
    },
    {
        "name": "lobo-macros",
        "paths": [
            "../lobo-macros/src",
        ],
    },
    {
        "name": "renderer",
        "paths": [
            "../renderer/src",
        ],
    },
    {
        "name": "renderer-cpu",
        "paths": [
            "../renderer-cpu/src",
        ],
    },
    {
        "name": "renderer-gpu",
        "paths": [
            "../renderer-gpu/src",
        ],
    },
]

IGNORED_DIRS = {
    ".git",
    ".idea",
    ".vscode",
    ".claude",
    "build",
    "target",
}


def count_code_lines(file_path):
    total_loc = 0
    in_block_comment = False

    with open(file_path, "r", encoding="utf-8") as file:
        for raw_line in file:
            line = raw_line.strip()
            if not line:
                continue

            i = 0
            has_code = False
            while i < len(line):
                if in_block_comment:
                    end = line.find("*/", i)
                    if end == -1:
                        i = len(line)
                        break
                    in_block_comment = False
                    i = end + 2
                    continue

                if line.startswith("//", i):
                    break

                if line.startswith("/*", i):
                    in_block_comment = True
                    i += 2
                    continue

                if not line[i].isspace():
                    has_code = True
                i += 1

            if has_code:
                total_loc += 1

    return total_loc


def collect_files(paths, extension):
    discovered = set()
    for folder in paths:
        if not os.path.isdir(folder):
            print(f"warning: missing folder, skipped: {folder}", file=sys.stderr)
            continue

        for root, dirs, files in os.walk(folder):
            dirs[:] = [d for d in dirs if d not in IGNORED_DIRS]
            for filename in files:
                if filename.endswith(extension):
                    full_path = os.path.realpath(os.path.join(root, filename))
                    discovered.add(full_path)
    return sorted(discovered)


def count_loc(paths, extension):
    per_path_rows = []
    for folder in paths:
        files = collect_files([folder], extension)
        total_loc = sum(count_code_lines(file_path) for file_path in files)
        per_path_rows.append((folder, total_loc, len(files)))

    files = collect_files(paths, extension)
    total_loc = sum(count_code_lines(file_path) for file_path in files)
    return total_loc, len(files), per_path_rows


def print_report(title, total_loc, total_files, per_path_rows):
    path_col_width = max(len("Path"), *(len(path) for path, _, _ in per_path_rows))
    loc_col_width = max(
        len("LOC"), *(len(str(loc)) for _, loc, _ in per_path_rows), len(str(total_loc))
    )
    files_col_width = max(
        len("Files"),
        *(len(str(file_count)) for _, _, file_count in per_path_rows),
        len(str(total_files)),
    )

    border = (
        f"+-{'-' * path_col_width}-+-{'-' * loc_col_width}-+-{'-' * files_col_width}-+"
    )

    print(f"{title} report")
    print(border)
    print(
        f"| {'Path'.ljust(path_col_width)} | {'LOC'.rjust(loc_col_width)} | {'Files'.rjust(files_col_width)} |"
    )
    print(border)
    for path, loc, file_count in per_path_rows:
        print(
            f"| {path.ljust(path_col_width)} | {str(loc).rjust(loc_col_width)} | {str(file_count).rjust(files_col_width)} |"
        )
    print(border)
    print(
        f"| {'TOTAL'.ljust(path_col_width)} | {str(total_loc).rjust(loc_col_width)} | {str(total_files).rjust(files_col_width)} |"
    )
    print(border)


if __name__ == "__main__":
    script_dir = os.path.dirname(os.path.abspath(__file__))
    overall_loc = 0
    overall_files = 0

    for check in LOC_CHECKS:
        folders = [
            os.path.normpath(os.path.join(script_dir, path)) for path in check["paths"]
        ]
        total_loc, total_files, per_path_rows = count_loc(folders, SOURCE_EXTENSION)

        print()
        print_report(f"{check['name']} (Rust)", total_loc, total_files, per_path_rows)
        overall_loc += total_loc
        overall_files += total_files

    print()
    print("=" * 56)
    print(f"Overall total LOC   : {overall_loc}")
    print(f"Overall total files : {overall_files}")
    print("=" * 56)
