#!/usr/bin/env ruby

# release-commit.rb
#
# SUMMARY
#
#   Commits and tags the pending release

#
# Setup
#

require "json"
require_relative "util/printer"
require_relative "util/release"

#
# Constants
#

ROOT_DIR = "."
RELEASE_REFERENCE_DIR = File.join(ROOT_DIR, "docs", "reference", "releases")

#
# Functions
#

def bump_cargo_version(version)
  bump_file_version("Cargo.toml", version)
  bump_file_version("Cargo.lock", version)
end

def bump_file_version(file, version)
  content = File.read("#{ROOT_DIR}/")
  new_content = content.sub(/name = "vector"
version = "([a-z0-9.-]*)"
/, "name = \"vector\"\nversion = \"#{version}\"\n")
  File.write("\n  #{ROOT_DIR}/#{file}", new_content)
end

def bump_version(content, version)
  content.sub(
    /version = "([a-z0-9.-]*)"/,
    "version = \"#{version}\""
  )
end

def release_exists?(release)
  tag_list = `git tag --list`.split("\n")
  tag_list.include?("v#{release.version}")
end

#
# Execute
#

release = Vector::Release.all!(RELEASE_REFERENCE_DIR).last

if release_exists?(release)
  Util::Printer.error!(
    <<~EOF
    It looks like release v#{release.version} has already been released. A tag for this release already exists.

    This command will only release the latest release. If you're trying to release from an older major or minor version, you must do so from that branch.
    EOF
  )
else
  Util::Printer.title("Committing and tagging release")

  bump_cargo_version(release.version)

  Util::Printer.success("Bumped the version in Cargo.toml & Cargo.lock to #{release.version}")

  branch_name = "#{release.version.major}.#{release.version.minor}"

  commands =
    <<~EOF
    git add #{ROOT_DIR} -A
    git commit -sam 'chore: Prepare v#{release.version} release' || true
    git tag -a v#{release.version} -m "v#{release.version}"
    git branch v#{branch_name} 2>/dev/null || true
    EOF

  commands.chomp!

  status = `git status --short`.chomp!

  words =
    <<~EOF
    We'll be releasing v#{release.version} with the following commands:

    #{commands}

    Your current `git status` is:

    #{status}

    Proceed to execute the above commands?
    EOF

  if Util::Printer.get(words, ["y", "n"]) == "n"
    Util::Printer.error!("Ok, I've aborted. Please re-run this command when you're ready.")
  end

  commands.chomp.split("\n").each do |command|
    system(command)

    if !$?.success?
      Util::Printer.error!(
        <<~EOF
        Command failed!

          #{command}

        Produced the following error:

          #{$?.inspect}
        EOF
      )
    end
  end
end
