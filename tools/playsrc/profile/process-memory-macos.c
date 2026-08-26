#include <errno.h>
#include <inttypes.h>
#include <libproc.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/proc_info.h>
#include <unistd.h>

int main(int argc, char **argv) {
    const uint64_t page_size = (uint64_t)getpagesize();
    for (int i = 1; i < argc; ++i) {
        char *end = NULL;
        long pid = strtol(argv[i], &end, 10);
        if (!end || *end || pid <= 0 || pid > INT32_MAX) return 2;
        struct proc_taskinfo task = {0};
        if (proc_pidinfo((int)pid, PROC_PIDTASKINFO, 0, &task, sizeof(task)) != sizeof(task)) continue;
        uint64_t address = 0, private_pages = 0;
        unsigned regions = 0;
        for (;;) {
            struct proc_regioninfo region = {0};
            int result = proc_pidinfo((int)pid, PROC_PIDREGIONINFO, address, &region, sizeof(region));
            if (result != sizeof(region)) break;
            private_pages += region.pri_private_pages_resident;
            ++regions;
            uint64_t next = region.pri_address + region.pri_size;
            if (next <= address || regions >= 1000000) return 3;
            address = next;
        }
        printf("%ld %" PRIu64 " ", pid, task.pti_resident_size);
        if (regions) printf("%" PRIu64 "\n", private_pages * page_size);
        else printf("null\n");
    }
    return 0;
}
