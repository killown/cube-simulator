from pathlib import Path


def convert_csv_to_md():
    """Converts CSV benchmark files to Markdown tables organized by mode."""
    base_path = Path.expanduser(Path("../"))
    modes = ["fifo", "mailbox"]

    for mode in modes:
        mode_dir = base_path / mode
        if not mode_dir.exists():
            continue

        output_file = f"../compositor-benchmarks-{mode}.md"

        with open(output_file, "w", encoding="utf-8") as out:
            csv_files = sorted(mode_dir.glob("*.csv"))

            for csv_path in csv_files:
                header_name = csv_path.stem.upper()
                out.write(f"## {header_name}\n\n")

                with open(csv_path, "r", encoding="utf-8") as f:
                    lines = f.readlines()
                    if not lines:
                        continue

                    headers = lines[0].strip().split(",")
                    out.write(f"| {' | '.join(headers)} |\n")
                    out.write(f"| {' | '.join(['---'] * len(headers))} |\n")

                    for line in lines[1:]:
                        values = line.strip().split(",")
                        if values:
                            out.write(f"| {' | '.join(values)} |\n")

                out.write("\n")


if __name__ == "__main__":
    convert_csv_to_md()
