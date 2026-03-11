#!/usr/bin/env bash
set -euo pipefail
umask 077

APP="lazycompass"
EXPECTED_SIGNING_FINGERPRINT="5D7EF1CB7FD9672A6D113B5C74502B609A660BAA"

MUTED='\033[0;2m'
RED='\033[0;31m'
ORANGE='\033[38;5;214m'
NC='\033[0m'

usage() {
  cat <<EOF
LazyCompass Installer

Usage: install.sh [options]

Options:
    -h, --help              Display this help message
    -v, --version <version> Install a specific version (GitHub release)
    -b, --binary <path>     Install from a local binary instead of downloading
        --repo <owner/repo> GitHub repo for releases
        --from-source       Build from source using cargo
        --no-modify-path    Don't modify shell config files (.zshrc, .bashrc, etc.)

Examples:
    ./install.sh
    ./install.sh --from-source
    ./install.sh --binary /path/to/lazycompass
    ./install.sh --repo org/lazycompass --version 0.1.0
EOF
}

requested_version=${VERSION:-}
binary_path=""
no_modify_path=false
from_source=false
repo_override=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    -v|--version)
      if [[ -n "${2:-}" ]]; then
        requested_version="$2"
        shift 2
      else
        echo -e "${RED}Error: --version requires a version argument${NC}" >&2
        exit 1
      fi
      ;;
    -b|--binary)
      if [[ -n "${2:-}" ]]; then
        binary_path="$2"
        shift 2
      else
        echo -e "${RED}Error: --binary requires a path argument${NC}" >&2
        exit 1
      fi
      ;;
    --repo)
      if [[ -n "${2:-}" ]]; then
        repo_override="$2"
        shift 2
      else
        echo -e "${RED}Error: --repo requires a value like owner/repo${NC}" >&2
        exit 1
      fi
      ;;
    --from-source)
      from_source=true
      shift
      ;;
    --no-modify-path)
      no_modify_path=true
      shift
      ;;
    *)
      echo -e "${ORANGE}Warning: Unknown option '$1'${NC}" >&2
      shift
      ;;
  esac
done

INSTALL_ROOT="${LAZYCOMPASS_INSTALL_ROOT:-$HOME/.lazycompass}"
INSTALL_DIR="$INSTALL_ROOT/bin"
mkdir -p "$INSTALL_DIR"

print_message() {
  local level=$1
  local message=$2
  local color=""

  case $level in
    info) color="${NC}" ;;
    warning) color="${NC}" ;;
    error) color="${RED}" ;;
  esac

  echo -e "${color}${message}${NC}"
}

script_dir() {
  cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd
}

resolve_repo_from_git() {
  local dir=$1
  if ! command -v git >/dev/null 2>&1; then
    return 1
  fi
  if ! git -C "$dir" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    return 1
  fi
  local url
  url=$(git -C "$dir" config --get remote.origin.url || true)
  if [[ -z "$url" ]]; then
    return 1
  fi
  url=${url#git@github.com:}
  url=${url#https://github.com/}
  url=${url%.git}
  if [[ "$url" == */* ]]; then
    echo "$url"
    return 0
  fi
  return 1
}

validate_repo() {
  local repo=$1
  if [[ ! "$repo" =~ ^[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$ ]]; then
    echo -e "${RED}Error: invalid repo '${repo}', expected owner/repo${NC}" >&2
    exit 1
  fi
}

install_from_binary() {
  if [ ! -f "$binary_path" ]; then
    echo -e "${RED}Error: Binary not found at ${binary_path}${NC}" >&2
    exit 1
  fi
  print_message info "${MUTED}Installing ${NC}$APP ${MUTED}from: ${NC}$binary_path"
  cp "$binary_path" "$INSTALL_DIR/$APP"
  chmod 755 "$INSTALL_DIR/$APP"
}

install_from_source() {
  if ! command -v cargo >/dev/null 2>&1; then
    echo -e "${RED}Error: cargo is required but not installed${NC}" >&2
    echo -e "Install Rust from https://www.rust-lang.org/tools/install" >&2
    exit 1
  fi
  local dir
  dir=$(script_dir)
  if [ ! -f "$dir/Cargo.toml" ]; then
    echo -e "${RED}Error: Cargo.toml not found in $dir${NC}" >&2
    exit 1
  fi
  print_message info "${MUTED}Installing ${NC}$APP ${MUTED}from source${NC}"
  cargo install --path "$dir" -p lazycompass --locked --root "$INSTALL_ROOT"
}

detect_target() {
  local raw_os
  raw_os=$(uname -s)
  local arch
  arch=$(uname -m)
  case "$raw_os/$arch" in
    Darwin*/x86_64) echo "darwin-x64" ;;
    Darwin*/aarch64|Darwin*/arm64) echo "darwin-arm64" ;;
    Linux*/x86_64) echo "linux-x64" ;;
    Linux*/*)
      echo -e "${RED}Unsupported Linux architecture for release installs: $arch${NC}" >&2
      echo -e "Use --from-source or a manual build instead." >&2
      exit 1
      ;;
    *)
      echo -e "${RED}Unsupported platform for release installs: $raw_os/$arch${NC}" >&2
      echo -e "Use --from-source or a manual build instead." >&2
      exit 1
      ;;
  esac
}

sha256_command() {
  if command -v sha256sum >/dev/null 2>&1; then
    echo "sha256sum"
    return 0
  fi
  if command -v shasum >/dev/null 2>&1; then
    echo "shasum -a 256"
    return 0
  fi
  return 1
}

verify_checksum() {
  local checksum_file=$1
  local asset_file=$2
  local tool
  if ! tool=$(sha256_command); then
    echo -e "${RED}Error: sha256sum or shasum is required to verify release assets${NC}" >&2
    exit 1
  fi

  local expected
  IFS=' ' read -r expected _ < "$checksum_file"
  if [[ -z "$expected" ]]; then
    echo -e "${RED}Error: checksum file is empty or invalid${NC}" >&2
    exit 1
  fi

  local actual
  actual="$($tool "$asset_file")"
  actual="${actual%% *}"
  if [[ "$expected" != "$actual" ]]; then
    echo -e "${RED}Error: checksum verification failed${NC}" >&2
    echo -e "Expected: $expected" >&2
    echo -e "Actual:   $actual" >&2
    exit 1
  fi
  print_message info "${MUTED}Checksum verified.${NC}"
}

normalize_fingerprint() {
  printf '%s' "$1" | tr -cd '[:alnum:]' | tr '[:lower:]' '[:upper:]'
}

write_release_signing_key() {
  local path=$1
  cat > "$path" <<'EOF'
-----BEGIN PGP PUBLIC KEY BLOCK-----

mQINBGmnYrkBEADLxOTiK6xcu+6Xe0wwj+flKIjF+55HwHRmFbq4JgFfcRCKG0Ut
ANV/fUcs/ZjKmz9qmbGY0diCruKLazP78vctFbpOwmJkQIm+t6rS1VwZ4QEeyu7i
88DXSQUSpt0s5eWFI+mFP2rnC8lNcokmgKDOJu7nntEjkNGyL/5LZmS/cUGP2kCw
JEZgY0FLIK2vlTWaPU2CjUdYjk+9RHwbgZc3vm6k7sgZ4I3b5qPZsbDuTvHWPeaL
+ihX44AKvrWCrq+pf11qQKr77uRsM83627i/fbAY2hicKcLzQSjORSiNhw3jOma0
PC1l0obUm2LX2JrXbSVIn3oFORf2dpgWBe0hCWOXcK3lyYHAzQWaPxfcM3d4M1bR
4E9nZJp157wFy3EN2ixWiXx9sMxfLotDSu3NOFmlVEKL3ikQ38XjsP02I3N34W2n
h1vJlVGdR1OnY9ppCAaX9O/wVtAW9bOHnDGKmG2O6SCf84csoc1kJ7VSCyOyPnkp
yoLakVFZj/KQscoZXenYVUb8hxwSHsYwa8ToP5uqw47AcmwYcFTauC4QbygSOnsr
O0qbVa9wq4sRj4DjEaq36ngDqgwNTOHI01IvDhCpLNJdvo8pOII0uwgbus3+Mgw0
ca0T/6Ft30risciFNENJ3mYEiJnMZu0YQa25lot0YizjwyZ35A5MCAjXGQARAQAB
tERMYXp5Q29tcGFzcyBSZWxlYXNlIFNpZ25pbmcgPGx1Y2Fzc2NhcmlvY2FAdXNl
cnMubm9yZXBseS5naXRodWIuY29tPokCUQQTAQoAOxYhBF1+8ct/2WcqbRE7XHRQ
K2CaZguqBQJpp2K5AhsDBQsJCAcCAiICBhUKCQgLAgQWAgMBAh4HAheAAAoJEHRQ
K2CaZguqRbkP/2jf1Pga4YgtKGazhCpAMuc8Oqd6mvvk//KXlzZS6SoyhwX+di7g
m74bMYABXjlofuuLG5T9WULob/YLBtJeZIewn1XoqtZ1pvOlUBi6LLWJPKv3/kKb
MNoxVdNq+Pos62HgFBqHKANmizY5vM3MfiAoSpM9uePA3zB1dk+oxzEgkrwrWWoe
ZdeFv2X0MUOizvvGhCQxbOAPCHs+IHv4/vhNxXbLcBGd+9NNtTAbBLU7BzyRjLQP
GmpeNOFW5xsFUYRncr6u6cy6Ujroh3VyB/ERFKEmW4LZm0Skj/V5aaimhVcFHb5M
QWQU83U5zg/6vJ1Zi4cxYbFCjyBL404JGzN9LK8dD+wmCdn/r8b2zjNrLB57m6vt
+ULc69/aWgplnplZY2a+xPXLBmKY+wK/Zn6uXtMecRl+aqKFfymazVmXuhcwzgGy
ZBqGucbc06SD6iYaZKash4VmXs5nhQxH3OJyFy9yURRljAoze/T0+fRJo1PcEujF
th4boXf1JIGmusOIg3vWN7tu/XiJkoXyy9JAbCeyJFAvyb1kpNuTqjb3FowOZVXi
BLOF+ND5Tbkd1/ojlt6QKSmoS9W9SVoQqO18L6f35xfjt3QK1Oki4sh6+qHagiNR
geTCCfAFc5tyKbK3sObOUQe9QG7rYx3BlVUdMVuAn1TIjb3B4XuJnPe5
=RDBb
-----END PGP PUBLIC KEY BLOCK-----
EOF
}

verify_signature() {
  local checksum_file=$1
  local sig_file=$2
  local temp_dir=$3
  if ! command -v gpg >/dev/null 2>&1; then
    print_message warning "gpg not found; skipping signature verification."
    return 0
  fi

  local gpg_home="$temp_dir/gpg-home"
  local key_file="$temp_dir/lazycompass-release-signing.asc"
  mkdir -p "$gpg_home"
  chmod 700 "$gpg_home"
  write_release_signing_key "$key_file"

  local fingerprint
  fingerprint=$(gpg --batch --show-keys --with-colons "$key_file" | awk -F: '$1=="fpr"{print $10; exit}')
  if [[ "$(normalize_fingerprint "$fingerprint")" != "$EXPECTED_SIGNING_FINGERPRINT" ]]; then
    echo -e "${RED}Error: bundled release signing key fingerprint does not match expected fingerprint${NC}" >&2
    exit 1
  fi

  if ! gpg --homedir "$gpg_home" --batch --import "$key_file" >/dev/null 2>&1; then
    echo -e "${RED}Error: failed to import bundled release signing key${NC}" >&2
    exit 1
  fi

  if ! gpg --homedir "$gpg_home" --batch --verify "$sig_file" "$checksum_file" >/dev/null 2>&1; then
    echo -e "${RED}Error: checksum signature verification failed${NC}" >&2
    exit 1
  fi
  print_message info "${MUTED}Signature verified.${NC}"
}

download_and_install() {
  local repo=$1
  validate_repo "$repo"
  local target
  target=$(detect_target)
  local asset="${APP}-${target}.tar.gz"
  local version=""
  local url=""
  local checksum_url=""
  local checksum_sig_url=""

  if [ -z "$requested_version" ]; then
    url="https://github.com/${repo}/releases/latest/download/${asset}"
  else
    requested_version="${requested_version#v}"
    url="https://github.com/${repo}/releases/download/v${requested_version}/${asset}"
    version="$requested_version"
  fi

  checksum_url="${url}.sha256"
  checksum_sig_url="${checksum_url}.sig"

  if ! command -v curl >/dev/null 2>&1; then
    echo -e "${RED}Error: curl is required but not installed${NC}" >&2
    exit 1
  fi

  if ! command -v tar >/dev/null 2>&1; then
    echo -e "${RED}Error: tar is required but not installed${NC}" >&2
    exit 1
  fi

  print_message info "${MUTED}Installing ${NC}$APP ${MUTED}from GitHub releases${NC}"
  if [ -n "$version" ]; then
    print_message info "${MUTED}Version:${NC} $version"
  fi

  local tmp_dir
  tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/lazycompass_install.XXXXXX")
  curl -# -L -o "$tmp_dir/$asset" "$url"

  local checksum_file="$tmp_dir/${asset}.sha256"
  local checksum_sig_file="$tmp_dir/${asset}.sha256.sig"
  if ! curl -fsL -o "$checksum_file" "$checksum_url"; then
    echo -e "${RED}Error: checksum unavailable for ${asset}${NC}" >&2
    exit 1
  fi
  print_message info "${MUTED}Verifying checksum...${NC}"
  verify_checksum "$checksum_file" "$tmp_dir/$asset"
  if curl -fsL -o "$checksum_sig_file" "$checksum_sig_url"; then
    print_message info "${MUTED}Verifying checksum signature...${NC}"
    verify_signature "$checksum_file" "$checksum_sig_file" "$tmp_dir"
  else
    print_message warning "Checksum signature unavailable; skipping signature verification."
  fi

  tar -xzf "$tmp_dir/$asset" -C "$tmp_dir"
  if [ ! -f "$tmp_dir/$APP" ]; then
    echo -e "${RED}Error: expected binary '$APP' in archive${NC}" >&2
    exit 1
  fi
  mv "$tmp_dir/$APP" "$INSTALL_DIR/$APP"
  chmod 755 "$INSTALL_DIR/$APP"
  rm -rf "$tmp_dir"
}

add_to_path() {
  local config_file=$1
  local command=$2

  if grep -Fxq "$command" "$config_file" 2>/dev/null; then
    print_message info "Command already exists in $config_file, skipping write."
  elif [[ -w $config_file ]]; then
    echo -e "\n# lazycompass" >> "$config_file"
    echo "$command" >> "$config_file"
    print_message info "${MUTED}Added ${NC}$APP ${MUTED}to PATH in ${NC}$config_file"
  else
    print_message warning "Manually add the directory to $config_file (or similar):"
    print_message info "  $command"
  fi
}

install_repo="${repo_override:-${LAZYCOMPASS_REPO:-lucasscarioca/lazycompass}}"
source_dir=$(script_dir)
if [[ -z "$install_repo" ]]; then
  install_repo=$(resolve_repo_from_git "$source_dir" || true)
fi

if [ -n "$binary_path" ]; then
  install_from_binary
elif [ "$from_source" = "true" ]; then
  install_from_source
elif [ -n "$requested_version" ] || [ -n "$repo_override" ]; then
  if [ -z "$install_repo" ]; then
    echo -e "${RED}Error: --version/--repo requires a GitHub repo (owner/repo)${NC}" >&2
    exit 1
  fi
  download_and_install "$install_repo"
elif [ -n "$install_repo" ]; then
  download_and_install "$install_repo"
elif [ -f "$source_dir/Cargo.toml" ]; then
  install_from_source
else
  echo -e "${RED}Error: no install method available${NC}" >&2
  echo -e "Use --from-source, --binary, or --repo" >&2
  exit 1
fi

if [[ "$no_modify_path" != "true" ]]; then
  XDG_CONFIG_HOME=${XDG_CONFIG_HOME:-$HOME/.config}
  current_shell=$(basename "$SHELL")
  case $current_shell in
    fish)
      config_files="$HOME/.config/fish/config.fish"
      ;;
    zsh)
      config_files="${ZDOTDIR:-$HOME}/.zshrc ${ZDOTDIR:-$HOME}/.zshenv $XDG_CONFIG_HOME/zsh/.zshrc $XDG_CONFIG_HOME/zsh/.zshenv"
      ;;
    bash)
      config_files="$HOME/.bashrc $HOME/.bash_profile $HOME/.profile $XDG_CONFIG_HOME/bash/.bashrc $XDG_CONFIG_HOME/bash/.bash_profile"
      ;;
    *)
      config_files="$HOME/.bashrc $HOME/.bash_profile $HOME/.profile $XDG_CONFIG_HOME/bash/.bashrc"
      ;;
  esac

  config_file=""
  for file in $config_files; do
    if [[ -f $file ]]; then
      config_file=$file
      break
    fi
  done

  if [[ -z $config_file ]]; then
    print_message warning "No shell config file found. Add to PATH manually:"
    print_message info "  export PATH=$INSTALL_DIR:\$PATH"
  elif [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    case $current_shell in
      fish)
        add_to_path "$config_file" "fish_add_path $INSTALL_DIR"
        ;;
      *)
        add_to_path "$config_file" "export PATH=$INSTALL_DIR:\$PATH"
        ;;
    esac
  fi
fi

echo -e ""
echo -e "${MUTED}Installed ${NC}$APP${MUTED}. Restart your shell if needed.${NC}"
