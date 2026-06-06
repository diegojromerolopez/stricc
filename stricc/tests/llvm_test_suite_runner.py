#!/usr/bin/env python3
import os
import sys
import subprocess
import glob
import shutil

WORKSPACE_ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
LLVM_SUITE_DIR = os.path.join(WORKSPACE_ROOT, "target", "llvm-test-suite")

def setup_llvm_suite():
    if os.path.exists(LLVM_SUITE_DIR):
        print("LLVM Test Suite already present.")
        return
    
    print("Cloning LLVM Test Suite via sparse checkout...")
    os.makedirs(LLVM_SUITE_DIR, exist_ok=True)
    
    # Clone only the SingleSource/UnitTests directory
    subprocess.run([
        "git", "clone", "--depth", "1", "--filter=blob:none", "--sparse",
        "https://github.com/llvm/llvm-test-suite.git", "llvm-src"
    ], cwd=LLVM_SUITE_DIR, check=True)
    
    llvm_src = os.path.join(LLVM_SUITE_DIR, "llvm-src")
    subprocess.run([
        "git", "sparse-checkout", "set", "SingleSource/UnitTests"
    ], cwd=llvm_src, check=True)

def find_test_files():
    ut_dir = os.path.join(LLVM_SUITE_DIR, "llvm-src", "SingleSource", "UnitTests")
    c_files = glob.glob(os.path.join(ut_dir, "*.c"))
    c_files.extend(glob.glob(os.path.join(ut_dir, "**", "*.c"), recursive=True))
    return sorted(list(set(c_files)))

def run_tests():
    setup_llvm_suite()
    test_files = find_test_files()
    print(f"Found {len(test_files)} LLVM C Unit Test files.")
    
    stricc_bin = os.path.join(WORKSPACE_ROOT, "target", "debug", "stricc")
    if not os.path.exists(stricc_bin):
        stricc_bin = os.path.join(WORKSPACE_ROOT, "target", "release", "stricc")
    
    if not os.path.exists(stricc_bin):
        print("Error: stricc binary not found. Please build the compiler first.")
        sys.exit(1)
        
    passed = 0
    failed_exec = 0
    failed_compile = 0
    skipped = 0
    
    temp_dir = os.path.join(WORKSPACE_ROOT, "target", "llvm-test-suite-temp")
    os.makedirs(temp_dir, exist_ok=True)
    
    for test_file in test_files:
        name = os.path.basename(test_file)
        
        # 1. Compile with host compiler (clang/gcc) to verify it compiles and runs on the host
        host_bin = os.path.join(temp_dir, f"host_{name}.out")
        try:
            host_compile = subprocess.run(["clang", "-O0", "-w", "-o", host_bin, test_file], capture_output=True, timeout=2)
            if host_compile.returncode != 0:
                skipped += 1
                continue
                
            host_run = subprocess.run([host_bin], capture_output=True, timeout=2)
            expected_exit_code = host_run.returncode
        except subprocess.TimeoutExpired:
            skipped += 1
            continue
            
        # 2. Compile with stricc
        stricc_out_bin = os.path.join(temp_dir, f"stricc_{name}.out")
        try:
            stricc_compile = subprocess.run([stricc_bin, "-o", stricc_out_bin, test_file], capture_output=True, timeout=2)
            if stricc_compile.returncode != 0:
                failed_compile += 1
                continue
        except subprocess.TimeoutExpired:
            print(f"FAIL: {name} (stricc compiler hang/timeout)")
            failed_compile += 1
            continue
            
        # 3. Run stricc binary
        try:
            stricc_run = subprocess.run([stricc_out_bin], capture_output=True, timeout=2)
            if stricc_run.returncode == expected_exit_code:
                passed += 1
            else:
                print(f"FAIL: {name} (Exit code mismatch: got {stricc_run.returncode}, expected {expected_exit_code})")
                if stricc_run.stderr:
                    print(f"  Stderr: {stricc_run.stderr.decode('utf-8', errors='ignore')}")
                failed_exec += 1
        except subprocess.TimeoutExpired:
            print(f"FAIL: {name} (stricc binary execution hang/timeout)")
            failed_exec += 1
            
    shutil.rmtree(temp_dir, ignore_errors=True)
    
    print("\n=== LLVM C Unit Tests Summary ===")
    print(f"Total Tests Found:      {len(test_files)}")
    print(f"Skipped (Host compile): {skipped}")
    print(f"Failed to Compile:     {failed_compile}")
    print(f"Failed to Execute:     {failed_exec}")
    print(f"Passed Execution:      {passed}")
    
    if failed_exec > 0:
        print(f"\nResult: FAILED ({failed_exec} correctness failures)")
        sys.exit(1)
    else:
        print("\nResult: SUCCESS (all compiled tests executed correctly)")
        sys.exit(0)

if __name__ == "__main__":
    run_tests()
