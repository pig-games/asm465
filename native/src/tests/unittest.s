.cpu "4510"
.enc "screen"

.section main
    #debug.setupDebugStats

    #ClearScreen 1
    jsr setLowerCase

    #debug.infoLn "unittest framework tests"
    #nl

    

    #nl
    #nl
    #debug.Stats
    jmp *

.endsection ; main