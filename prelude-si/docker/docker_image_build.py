#!/usr/bin/env python3
"""
Invokes a `docker image build`.
"""
import argparse
import os
import subprocess
import json
import sys
from enum import Enum, EnumMeta
from typing import Any, Dict, List


# A slightly more Rust-y feeling enum
# Thanks to: https://stackoverflow.com/a/65225753
class MetaEnum(EnumMeta):

    def __contains__(self: type[Any], member: object) -> bool:
        try:
            self(member)
        except ValueError:
            return False
        return True


class BaseEnum(Enum, metaclass=MetaEnum):
    pass


class DockerArchitecture(BaseEnum):
    Amd64 = "amd64"
    Arm64v8 = "arm64v8"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--git-info-program",
        required=True,
        help="Path to the `git_info.py` program",
    )
    parser.add_argument(
        "--archive-out-file",
        required=True,
        help="Path to write the image archive file",
    )
    parser.add_argument(
        "--metadata-out-file",
        required=True,
        help="Path to write the metadata JSON file",
    )
    parser.add_argument(
        "--tags-out-file",
        required=True,
        help="Path to write the tags JSON file",
    )
    parser.add_argument(
        "--docker-context-dir",
        required=True,
        help="Path to Docker context directory",
    )
    parser.add_argument(
        "--image-name",
        required=True,
        help="Name of image to build",
    )
    parser.add_argument(
        "--build-arg",
        action="append",
        help="Build arg options passed to `docker build`",
    )
    parser.add_argument(
        "--author",
        required=True,
        help="Image author to be used in image metadata",
    )
    parser.add_argument(
        "--source-url",
        required=True,
        help="Source code URL to be used in image metadata",
    )
    parser.add_argument(
        "--license",
        required=True,
        help="Image license to be used in image metadata",
    )

    return parser.parse_args()


def main() -> int:
    args = parse_args()

    git_info = load_git_info(args.git_info_program)
    architecture = detect_architecture()
    metadata = compute_metadata(
        git_info,
        architecture,
        args.image_name,
        args.author,
        args.source_url,
        args.license,
    )

    tags = [
        "{}:{}-{}".format(
            metadata.get("name"),
            metadata.get("org.opencontainers.image.version"),
            architecture.value,
        ),
        "{}:sha-{}-{}".format(
            metadata.get("name"),
            metadata.get("org.opencontainers.image.revision"),
            architecture.value,
        ),
    ]

    build_image(
        args.docker_context_dir,
        metadata,
        args.build_arg or [],
        tags,
    )
    write_metadata(args.metadata_out_file, metadata)
    write_tags(args.tags_out_file, tags)
    write_archive(args.archive_out_file, tags)

    return 0


def build_image(
    cwd: str,
    metadata: Dict[str, str | str],
    build_args: List[str],
    tags: List[str],
):
    cmd = [
        "docker",
        "image",
        "build",
    ]
    for key, value in metadata.items():
        cmd.append("--label")
        cmd.append(f"{key}={value}")
    for build_arg in build_args:
        cmd.append("--build-arg")
        cmd.append(build_arg)
    for tag in tags:
        cmd.append("--tag")
        cmd.append(tag)
    cmd.append("--file")
    cmd.append("Dockerfile")
    cmd.append(".")

    print("--- Build image with: {}".format(" ".join(cmd)))
    subprocess.run(cmd, cwd=cwd).check_returncode()


def write_metadata(output: str, metadata: Dict[str, str]):
    with open(output, "w") as file:
        json.dump(metadata, file, sort_keys=True)


def write_tags(output: str, tags: List[str]):
    with open(output, "w") as file:
        json.dump(tags, file, sort_keys=True)


def write_archive(output: str, tags: List[str]):
    cmd = [
        "docker",
        "save",
        "--output",
        output,
    ]
    cmd.extend(tags)

    print("--- Creating image archive with: {}".format(" ".join(cmd)))
    subprocess.run(cmd).check_returncode()


# Possible machine architecture detection comes from reading the Rustup shell
# script installer--thank you for your service!
# See: https://github.com/rust-lang/rustup/blob/master/rustup-init.sh
def detect_architecture() -> DockerArchitecture:
    machine = os.uname().machine

    if (machine == "amd64" or machine == "x86_64" or machine == "x86-64"
            or machine == "x64"):
        return DockerArchitecture.Amd64
    elif (machine == "arm64" or machine == "aarch64" or machine == "arm64v8"):
        return DockerArchitecture.Arm64v8
    else:
        print(
            f"xxx Failed to determine architecure or unsupported: {machine}",
            file=sys.stderr,
        )
        sys.exit(1)


def load_git_info(git_info_program: str) -> Dict[str, str | int | bool]:
    result = subprocess.run([git_info_program], capture_output=True)
    result.check_returncode()
    return json.loads(result.stdout)


def compute_metadata(
    git_info: Dict[str, str | int | bool],
    architecture: DockerArchitecture,
    image_name: str,
    author: str,
    source_url: str,
    license: str,
) -> Dict[str, str]:
    created = git_info.get("committer_date_strict_iso8601")
    revision = git_info.get("commit_hash")
    build_version = "{}-sha.{}".format(
        git_info.get("cal_ver"),
        git_info.get("abbreviated_commit_hash"),
    )

    commit_url = "{}/commit/{}".format(
        source_url.removesuffix(".git"),
        revision,
    )

    if git_info.get("is_dirty") and isinstance(revision, str) and isinstance(
            build_version, str):
        revision += "-dirty"
        build_version += "-dirty"

    image_url = ("https://hub.docker.com/r/{}/" +
                 "tags?page=1&ordering=last_updated&name={}-{}").format(
                     image_name,
                     build_version,
                     architecture.value,
                 )

    metadata = {
        "name": image_name,
        "maintainer": author,
        "org.opencontainers.image.version": build_version,
        "org.opencontainers.image.authors": author,
        "org.opencontainers.image.licenses": license,
        "org.opencontainers.image.source": source_url,
        "org.opencontainers.image.revision": revision,
        "org.opencontainers.image.created": created,
        "com.systeminit.image.architecture": architecture.value,
        "com.systeminit.image.image_url": image_url,
        "com.systeminit.image.commit_url": commit_url,
    }

    return metadata


if __name__ == "__main__":
    sys.exit(main())
