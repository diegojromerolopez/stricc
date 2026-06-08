void page_overflow(int *p) {
    // Write far beyond the page boundary to trigger a SIGSEGV in the guard page.
    // The data buffer size in main is 4 bytes.
    // LHS guard page is PROT_NONE. RHS guard page is at (num_data_pages + 1) * page_size.
    // In 16KB page size configuration, writing at offset 20,000 bytes (index 5000)
    // lands exactly inside the RHS guard page (offset 32768 to 49152 relative to mmap start).
    // In 4KB page size configuration, it lands past the RHS guard page in unmapped memory.
    // Both reliably trigger a page fault SEGSEGV.
    p[5000] = 999;
}
