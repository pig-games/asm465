.include "debug_macros.h"

.enc "screen"

.section main
    .dbg.setupDebugStats

    .ClearScreen 1
    jsr setLowerCase

    .dbg.info "Unittest framework tests!n"

    .nl
    .nl
    .dbg.Stats
    jmp *

.endsection ; main