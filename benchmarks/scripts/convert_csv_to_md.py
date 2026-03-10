from pathlib import Path


def convert_csv_to_md():
    """Converts CSV benchmark files to Markdown tables from compositor-based directories."""
    base_path = (
        Path(__file__).parent.parent if "scripts" in str(Path(__file__)) else Path("./")
    )

    modes = ["fifo", "mailbox"]

    for mode in modes:
        output_file = base_path / f"compositor-benchmarks-{mode}.md"

        # Collect all directories that aren't 'scripts', 'fifo', or 'mailbox'
        # These are our compositor folders (sway, gnome, etc.)
        compositor_dirs = sorted(
            [
                d
                for d in base_path.iterdir()
                if d.is_dir() and d.name not in ["scripts", "fifo", "mailbox", ".git"]
            ]
        )

        with open(output_file, "w", encoding="utf-8") as out:
            out.write(f"# Benchmarks: {mode.upper()}\n\n")

            for comp_dir in compositor_dirs:
                csv_path = comp_dir / f"{mode}.csv"

                if not csv_path.exists():
                    continue

                header_name = comp_dir.name.upper()
                out.write(f"## {header_name}\n\n")

                with open(csv_path, "r", encoding="utf-8") as f:
                    lines = f.readlines()
                    if not lines:
                        continue

                    # Filter out any non-CSV metadata lines if they exist in the file
                    # Only start processing once we find the header row
                    csv_start_idx = 0
                    for i, line in enumerate(lines):
                        if "FPS,MIN,MAX" in line:
                            csv_start_idx = i
                            break

                    headers = lines[csv_start_idx].strip().split(",")
                    out.write(f"| {' | '.join(headers)} |\n")
                    out.write(f"| {' | '.join(['---'] * len(headers))} |\n")

                    for line in lines[csv_start_idx + 1 :]:
                        line = line.strip()
                        if not line or "," not in line:
                            continue
                        values = line.split(",")
                        out.write(f"| {' | '.join(values)} |\n")

                out.write("\n")


if __name__ == "__main__":
    convert_csv_to_md()
