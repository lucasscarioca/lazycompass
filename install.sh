#!/usr/bin/env bash
set -euo pipefail

APP="lazycompass"

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
  local os
  case "$raw_os" in
    Darwin*) os="darwin" ;;
    Linux*) os="linux" ;;
    *)
      echo -e "${RED}Unsupported OS: $raw_os${NC}" >&2
      exit 1
      ;;
  esac

  local arch
  arch=$(uname -m)
  case "$arch" in
    aarch64) arch="arm64" ;;
    arm64) arch="arm64" ;;
    x86_64) arch="x64" ;;
    *)
      echo -e "${RED}Unsupported architecture: $arch${NC}" >&2
      exit 1
      ;;
  esac

  echo "$os-$arch"
}

download_and_install() {
  local repo=$1
  local target
  target=$(detect_target)
  local asset="${APP}-${target}.tar.gz"
  local version=""
  local url=""

  if [ -z "$requested_version" ]; then
    url="https://github.com/${repo}/releases/latest/download/${asset}"
  else
    requested_version="${requested_version#v}"
    url="https://github.com/${repo}/releases/download/v${requested_version}/${asset}"
    version="$requested_version"
  fi

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

  local tmp_dir="${TMPDIR:-/tmp}/lazycompass_install_$$"
  mkdir -p "$tmp_dir"
  curl -# -L -o "$tmp_dir/$asset" "$url"
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

install_repo="${repo_override:-${LAZYCOMPASS_REPO:-}}"
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
elif [ -f "$source_dir/Cargo.toml" ]; then
  install_from_source
elif [ -n "$install_repo" ]; then
  download_and_install "$install_repo"
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
