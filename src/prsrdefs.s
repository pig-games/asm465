datasize .word lookupend-mnemonics
tokensize .word lookupend-toktomnem
size     .word toktomnem-addrmgroups

mnemonics
mnadc    .text "adc@"
         .byte $61,gr01-addrmgroups
mnand    .text "and@"
         .byte $21,gr01-addrmgroups
mnasl    .text "asl@"
         .byte $06,gr02-addrmgroups
mnasr    .text "asr@"
         .byte $43,gr03-addrmgroups
mnbcc    .text "bcc@"
         .byte $90,gr06-addrmgroups
mnbcs    .text "bcs@"
         .byte $b0,gr06-addrmgroups
mnbeq    .text "beq@"
         .byte $f0,gr06-addrmgroups
mnbit    .text "bit@"
         .byte $24,gr07-addrmgroups
mnbmi    .text "bmi@"
         .byte $30,gr06-addrmgroups
mnbne    .text "bne@"
         .byte $d0,gr06-addrmgroups
mnbpl    .text "bpl@"
         .byte $10,gr06-addrmgroups
mnbra    .text "bra@"
         .byte $80,gr06-addrmgroups
mnbrk    .text "brk@"
         .byte $00,gr08-addrmgroups
mnbsr    .text "bsr@"
         .byte $65,gr09-addrmgroups
mnbvc    .text "bvc@"
         .byte $50,gr06-addrmgroups
mnbvs    .text "bvs@"
         .byte $70,gr06-addrmgroups
mnclc    .text "clc@"
         .byte $18,gr08-addrmgroups
mncld    .text "cld@"
         .byte $d8,gr08-addrmgroups
mncle    .text "cle@"
         .byte $02,gr08-addrmgroups
mncli    .text "cli@"
         .byte $58,gr08-addrmgroups
mnclv    .text "clv@"
         .byte $b8,gr08-addrmgroups
mncmp    .text "cmp@"
         .byte $c1,gr01-addrmgroups
mncpx    .text "cpx@"
         .byte $e0,gr0a-addrmgroups
mncpy    .text "cpy@"
         .byte $c0,gr0a-addrmgroups
mncpz    .text "cpz@"
         .byte $c2,gr0a-addrmgroups
mndec    .text "dec@"
         .byte $ff,gr0b-addrmgroups
mndew    .text "dew@"
         .byte $c3,gr0c-addrmgroups
mndex    .text "dex@"
         .byte $ca,gr08-addrmgroups
mndey    .text "dey@"
         .byte $88,gr08-addrmgroups
mndez    .text "dez@"
         .byte $3b,gr08-addrmgroups
mneom    .text "dez@"
         .byte $ea,gr08-addrmgroups
mneor    .text "eor@"
         .byte $41,gr01-addrmgroups
mninc    .text "inc@"
         .byte $1a,gr0b-addrmgroups
mninw    .text "inw@"
         .byte $e3,gr0c-addrmgroups
mninx    .text "inx@"
         .byte $e8,gr08-addrmgroups
mniny    .text "iny@"
         .byte $c8,gr08-addrmgroups
mninz    .text "inz@"
         .byte $1b,gr08-addrmgroups
mnjmp    .text "jmp@"
         .byte $4c,gr0d-addrmgroups
mnjsr    .text "jsr@"
         .byte $20,gr0d-addrmgroups
mnlda    .text "lda@"
         .byte $a1,gr01-addrmgroups
mnldx    .text "ldx@"
         .byte $a2,gr0e-addrmgroups
mnldy    .text "ldy@"
         .byte $a0,gr0e-addrmgroups
mnlsr    .text "lsr@"
         .byte $46,gr02-addrmgroups
mnmap    .text "map@"
         .byte $5c,gr08-addrmgroups
mnneg    .text "neg@"
         .byte $42,gr10-addrmgroups
mnora    .text "ora@"
         .byte $ff,gr01-addrmgroups
mnpha    .text "pha@"
         .byte $48,gr08-addrmgroups
mnphp    .text "php@"
         .byte $08,gr08-addrmgroups
mnphw    .text "phw@"
         .byte $f4,gr11-addrmgroups
mnphx    .text "phx@"
         .byte $da,gr08-addrmgroups
mnphy    .text "phy@"
         .byte $5a,gr08-addrmgroups
mnphz    .text "phz@"
         .byte $db,gr08-addrmgroups
mnpla    .text "pla@"
         .byte $68,gr08-addrmgroups
mnplp    .text "plp@"
         .byte $28,gr08-addrmgroups
mnplx    .text "plx@"
         .byte $ff,gr08-addrmgroups
mnply    .text "ply@"
         .byte $ff,gr08-addrmgroups
mnplz    .text "plz@"
         .byte $fb,gr08-addrmgroups
mnend

addrmgroups
gr01     .byte amxin,$00,$50
         .byte ambp,$04,$30
         .byte amimm,$04,$20
         .byte amabs,$04,$40
         .byte aminy,$04,$50
         .byte aminz,$01,$50
         .byte ambpx,$03,$30
         .byte amaby,$04,$40
         .byte amabx,$04,$40
gr02     .byte ambp,$00,$40
         .byte amacc,$04,$10
         .byte amabs,$04,$50
         .byte ambpx,$08,$40
         .byte amabx,$08,$40
gr03     .byte amacc,$00,$10
         .byte ambp,$01,$40
         .byte ambpx,$0a,$40
gr04     .byte amabs,$00,$00
gr05     .byte ambpr,$00,$50
gr06     .byte amrel,$00,$20
         .byte amrlw,$03,$30
gr07     .byte ambp,$00,$40
         .byte amabs,$04,$50
         .byte ambpx,$08,$40
         .byte amabx,$08,$40
         .byte amimm,$4a,$00
gr08     .byte amimp,$00,$70
gr09     .byte amrlw,$00,$30
gr0a     .byte amimm,$00,$00
         .byte ambp,$04,$00
         .byte amabs,$08,$00
gr0b     .byte amacc,$00,$10
         .byte ambp,$8c,$00
         .byte amabs,$08,$00
         .byte ambpx,$08,$00
         .byte amabx,$08,$00
gr0c     .byte ambp,$00,$00
gr0d     .byte amabs,$00,$30
         .byte amind,$20,$50
         .byte amxin,$10,$00
gr0e     .byte amimm,$00,$00
         .byte ambp,$04,$00
         .byte amabs,$08,$00
         .byte ambpx,$08,$00
         .byte amabx,$08,$00
gr0f     .byte amimm,$00,$00
         .byte ambp,$08,$00
         .byte amabs,$10,$00
gr10     .byte amacc,$00,$10
         .byte amimw,$00,$00
         .byte amabs,$08,$00

toktomnem
         .byte $00,<mnbrk,>mnbrk
         .byte $02,<mncle,>mncle
         .byte $06,<mnasl,>mnasl
         .byte $0f,<mnbr0,>mnbr0
         .byte $10,<mnbpl,>mnbpl
         .byte $18,<mnclc,>mnclc
         .byte $1f,<mnbr1,>mnbr1
         .byte $21,<mnand,>mnand
         .byte $24,<mnbit,>mnbit
         .byte $2f,<mnbr2,>mnbr2
         .byte $30,<mnbmi,>mnbmi
         .byte $3a,<mndec,>mndec
         .byte $3f,<mnbr3,>mnbr3
         .byte $43,<mnasr,>mnasr
         .byte $4f,<mnbr4,>mnbr4
         .byte $50,<mnbvc,>mnbvc
         .byte $58,<mncli,>mncli
         .byte $5f,<mnbr5,>mnbr5
         .byte $61,<mnadc,>mnadc
         .byte $65,<mnbsr,>mnbsr
         .byte $6f,<mnbr6,>mnbr6
         .byte $70,<mnbvs,>mnbvs
         .byte $7f,<mnbr7,>mnbr7
         .byte $80,<mnbra,>mnbra
         .byte $8f,<mnbs0,>mnbs0
         .byte $90,<mnbcc,>mnbcc
         .byte $9f,<mnbs1,>mnbs1
         .byte $af,<mnbs2,>mnbs2
         .byte $b0,<mnbcs,>mnbcs
         .byte $b8,<mnclv,>mnclv
         .byte $bf,<mnbs3,>mnbs3
         .byte $c0,<mncpy,>mncpy
         .byte $c1,<mncmp,>mncmp
         .byte $c2,<mncpz,>mncpz
         .byte $c3,<mndew,>mndew
         .byte $ca,<mndex,>mndex
         .byte $cb,<mnasw,>mnasw
         .byte $cf,<mnbs4,>mnbs4
         .byte $d0,<mnbne,>mnbne
         .byte $d8,<mncld,>mncld
         .byte $df,<mnbs5,>mnbs5
         .byte $e0,<mncpx,>mncpx
         .byte $ef,<mnbs6,>mnbs6
         .byte $f0,<mnbeq,>mnbeq
         .byte $ff,<mnbs7,>mnbs7
         .byte $88,<mndey,>mndey
         .byte $3b,<mndez,>mndez
         .byte $ea,<mneom,>mneom
         .byte $41,<mneor,>mneor
         .byte $1a,<mninc,>mninc
         .byte $e3,<mninw,>mninw
         .byte $e8,<mninx,>mninx
         .byte $c8,<mniny,>mniny
         .byte $1b,<mninz,>mninz
         .byte $4c,<mnjmp,>mnjmp
         .byte $20,<mnjsr,>mnjsr
         .byte $a1,<mnlda,>mnlda
         .byte $a2,<mnldx,>mnldx
         .byte $a0,<mnldy,>mnldy
         .byte $a3,<mnldz,>mnldz
         .byte $46,<mnlsr,>mnlsr
         .byte $5c,<mnmap,>mnmap
         .byte $42,<mnneg,>mnneg
         .byte $01,<mnora,>mnora
         .byte $48,<mnpha,>mnpha
         .byte $08,<mnphp,>mnphp
         .byte $f4,<mnphw,>mnphw
         .byte $da,<mnphx,>mnphx
         .byte $5a,<mnphy,>mnphy
         .byte $db,<mnphz,>mnphz
         .byte $68,<mnpla,>mnpla
         .byte $28,<mnplp,>mnplp
         .byte $fa,<mnplx,>mnplx
         .byte $7a,<mnply,>mnply

