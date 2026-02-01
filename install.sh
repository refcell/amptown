#!/usr/bin/env bash
#
# amptown installer
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/andreasbigger/amptown/main/install.sh | bash
#
# Or with a custom install directory:
#   curl -fsSL https://raw.githubusercontent.com/andreasbigger/amptown/main/install.sh | bash -s -- --dir ~/.local/bin
#

set -euo pipefail

# Configuration
REPO="andreasbigger/amptown"
BRANCH="main"
SCRIPT_NAME="amptown"
RAW_URL="https://raw.githubusercontent.com/${REPO}/${BRANCH}/${SCRIPT_NAME}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $*"; }
log_success() { echo -e "${GREEN}[OK]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }

# Detect install directory
detect_install_dir() {
    local dirs=(
        "$HOME/.local/bin"
        "$HOME/bin"
        "/usr/local/bin"
    )
    
    for dir in "${dirs[@]}"; do
        if [[ -d "$dir" ]] && [[ -w "$dir" ]]; then
            echo "$dir"
            return 0
        fi
    done
    
    # Default to ~/.local/bin, will create it
    echo "$HOME/.local/bin"
}

# Check if directory is in PATH
in_path() {
    local dir="$1"
    echo "$PATH" | tr ':' '\n' | grep -q "^${dir}$"
}

# Main installer
main() {
    local install_dir=""
    
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            -d|--dir)
                install_dir="$2"
                shift 2
                ;;
            -h|--help)
                echo "Usage: install.sh [OPTIONS]"
                echo ""
                echo "OPTIONS:"
                echo "  -d, --dir PATH    Install directory (default: ~/.local/bin)"
                echo "  -h, --help        Show this help"
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                exit 1
                ;;
        esac
    done
    
    # Set install directory
    if [[ -z "$install_dir" ]]; then
        install_dir=$(detect_install_dir)
    fi
    
    echo ""
    echo -e "${BOLD}╭──────────────────────────────────────╮${NC}"
    echo -e "${BOLD}│         amptown installer            │${NC}"
    echo -e "${BOLD}╰──────────────────────────────────────╯${NC}"
    echo ""
    
    # Check for required tools
    if ! command -v curl &> /dev/null; then
        log_error "curl is required but not installed"
        exit 1
    fi
    
    # Create install directory if needed
    if [[ ! -d "$install_dir" ]]; then
        log_info "Creating directory: $install_dir"
        mkdir -p "$install_dir"
    fi
    
    # Download the script
    local target="${install_dir}/${SCRIPT_NAME}"
    log_info "Downloading amptown from GitHub..."
    
    if curl -fsSL "$RAW_URL" -o "$target"; then
        chmod +x "$target"
        log_success "Installed to: $target"
    else
        log_error "Failed to download amptown"
        log_error "URL: $RAW_URL"
        exit 1
    fi
    
    # Check if in PATH
    if ! in_path "$install_dir"; then
        log_warn "$install_dir is not in your PATH"
        echo ""
        echo "Add it to your shell config:"
        echo ""
        echo "  # For bash (~/.bashrc):"
        echo "  export PATH=\"\$PATH:$install_dir\""
        echo ""
        echo "  # For zsh (~/.zshrc):"
        echo "  export PATH=\"\$PATH:$install_dir\""
        echo ""
        echo "Then reload your shell or run:"
        echo "  source ~/.bashrc  # or ~/.zshrc"
        echo ""
    fi
    
    # Verify installation
    echo ""
    log_success "amptown installed successfully!"
    echo ""
    echo -e "${BOLD}Quick start:${NC}"
    echo "  amptown ~/path/to/your/repo    # Start 6 agents on a repo"
    echo "  amptown --status               # Check agent status"
    echo "  amptown --help                 # Show all options"
    echo ""
    echo -e "${BOLD}Requirements:${NC}"
    echo "  • gastown (gt) - brew install gastown"
    echo "  • amp          - https://ampcode.com"
    echo "  • tmux         - brew install tmux"
    echo ""
}

main "$@"
