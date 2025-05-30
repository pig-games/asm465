.enc "screen"

.section main
    .dbg.setupDebugStats

    .ClearScreen 1
    jsr setLowerCase

    .dbg.info "unittest framework tests!n"

    .nl
    .nl
    .dbg.Stats
    jmp *

.endsection ; main