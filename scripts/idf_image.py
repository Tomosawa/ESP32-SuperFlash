"""Dispatch the bundled ESP-IDF host generators without a Python installation."""

import sys

from fatfs_utils.utils import get_args_for_partition_generator
from fatfsgen import main as fatfs
from spiffsgen import main as spiffs
from wl_fatfsgen import WLFATFS


def fatfs_wl():
    args = get_args_for_partition_generator("Create a wear-levelled FAT image", wl=True)
    image = WLFATFS(
        size=args.partition_size,
        sector_size=args.sector_size,
        fat_tables_cnt=args.fat_count,
        sectors_per_cluster=args.sectors_per_cluster,
        explicit_fat_type=args.fat_type,
        long_names_enabled=args.long_name_support,
        use_default_datetime=args.use_default_datetime,
        root_entry_count=args.root_entry_count,
        wl_mode=args.wl_mode,
    )
    image.plain_fatfs.generate(args.input_directory)
    image.init_wl()
    image.wl_write_filesystem(args.output_file)


if __name__ == "__main__":
    generators = {"fatfs": fatfs, "fatfs-wl": fatfs_wl, "spiffs": spiffs}
    if len(sys.argv) < 2 or sys.argv[1] not in generators:
        sys.exit("Usage: idf-image {fatfs|fatfs-wl|spiffs} [args]")
    generators[sys.argv.pop(1)]()
