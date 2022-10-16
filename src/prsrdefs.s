mnemsize .word (mnend-mnemonics)/6
toksize  .word tokend-toktomnm

mnemonics
mnadc    .text "adc@"
         .byte $61,gr01-addrmgroups
mnand    .text "and@"
         .byte $21,gr01-addrmgroups
mnasl    .text "asl@"
         .byte $06,gr02-addrmgroups
mnbcc    .text "bcc@"
         .byte $90,gr04-addrmgroups
mnbcs    .text "bcs@"
         .byte $b0,gr04-addrmgroups
mnbeq    .text "beq@"
         .byte $f0,gr04-addrmgroups
mnbit    .text "bit@"
         .byte $24,gr05-addrmgroups
mnbmi    .text "bmi@"
         .byte $30,gr04-addrmgroups
mnbne    .text "bne@"
         .byte $d0,gr04-addrmgroups
mnbpl    .text "bpl@"
         .byte $10,gr04-addrmgroups
mnbrk    .text "brk@"
         .byte $00,gr06-addrmgroups
mnbvc    .text "bvc@"
         .byte $50,gr04-addrmgroups
mnbvs    .text "bvs@"
         .byte $70,gr04-addrmgroups
mnclc    .text "clc@"
         .byte $18,gr06-addrmgroups
mncld    .text "cld@"
         .byte $d8,gr06-addrmgroups
mncli    .text "cli@"
         .byte $58,gr06-addrmgroups
mnclv    .text "clv@"
         .byte $b8,gr06-addrmgroups
mncmp    .text "cmp@"
         .byte $c1,gr01-addrmgroups
mncpx    .text "cpx@"
         .byte $e0,gr07-addrmgroups
mncpy    .text "cpy@"
         .byte $c0,gr07-addrmgroups
mndec    .text "dec@"
         .byte $c6,gr07-addrmgroups
mndex    .text "dex@"
         .byte $ca,gr06-addrmgroups
mndey    .text "dey@"
         .byte $88,gr06-addrmgroups
mneor    .text "eor@"
         .byte $41,gr01-addrmgroups
mninc    .text "inc@"
         .byte $1a,gr08-addrmgroups
mninx    .text "inx@"
         .byte $e8,gr06-addrmgroups
mniny    .text "iny@"
         .byte $c8,gr06-addrmgroups
mnjmp    .text "jmp@"
         .byte $4c,gr09-addrmgroups
mnjsr    .text "jsr@"
         .byte $20,gr03-addrmgroups
mnlda    .text "lda@"
         .byte $a1,gr01-addrmgroups
mnldx    .text "ldx@"
         .byte $a2,gr10-addrmgroups
mnldy    .text "ldy@"
         .byte $a0,gr10-addrmgroups
mnlsr    .text "lsr@"
         .byte $46,gr02-addrmgroups
mnnop    .text "nop@"
         .byte $ea,gr06-addrmgroups
mnora    .text "ora@"
         .byte $01,gr01-addrmgroups
mnpha    .text "pha@"
         .byte $48,gr06-addrmgroups
mnphp    .text "php@"
         .byte $08,gr06-addrmgroups
mnpla    .text "pla@"
         .byte $68,gr06-addrmgroups
mnplp    .text "plp@"
         .byte $28,gr06-addrmgroups
mnrol    .text "rol@"
         .byte $26,gr02-addrmgroups
mnror    .text "ror@"
         .byte $66,gr02-addrmgroups
mnrti    .text "rti@"
         .byte $40,gr06-addrmgroups
mnrts    .text "rts@"
         .byte $60,gr06-addrmgroups
mnsbc    .text "sbc@"
         .byte $e1,gr01-addrmgroups
mnsec    .text "sec@"
         .byte $38,gr06-addrmgroups
mnsed    .text "sed@"
         .byte $f8,gr06-addrmgroups
mnsei    .text "sei@"
         .byte $78,gr06-addrmgroups
mnsta    .text "sta@"
         .byte $81,gr08-addrmgroups
mnstx    .text "stx@"
         .byte $86,gr11-addrmgroups
mnsty    .text "sty@"
         .byte $84,gr12-addrmgroups
mntax    .text "tax@"
         .byte $aa,gr06-addrmgroups
mntay    .text "tay@"
         .byte $a8,gr06-addrmgroups
mntsx    .text "tsx@"
         .byte $ba,gr06-addrmgroups
mntxa    .text "tax@"
         .byte $8a,gr06-addrmgroups
mntxs    .text "txs@"
         .byte $9a,gr06-addrmgroups
mntya    .text "tya@"
         .byte $98,gr06-addrmgroups
mnend

addrmgroups
gr01     .byte amxin,$00,$50
         .byte amzp,$04,$30
         .byte amimm,$04,$20
         .byte amabs,$04,$40
         .byte aminy,$04,$50
         .byte amzpx,$04,$30
         .byte amaby,$04,$40
         .byte amabx,$04,$40
gr02     .byte amzp,$00,$40
         .byte amacc,$04,$10
         .byte amabs,$04,$50
         .byte amzpx,$08,$40
         .byte amabx,$08,$40
gr03     .byte amabs,$00,$60
gr04     .byte amrel,$00,$20
gr05     .byte amzp,$00,$30
         .byte amabs,$04,$40
gr06     .byte amimp,$00,$70
gr07     .byte amimm,$00,$20
         .byte amzp,$04,$30
         .byte amabs,$08,$40
gr08     .byte amzp,$8c,$50
         .byte amabs,$08,$60
         .byte amzpx,$08,$60
         .byte amabx,$08,$70
gr09     .byte amabs,$00,$30
         .byte amind,$20,$50
gr10     .byte amimm,$00,$20
         .byte amzp,$04,$30
         .byte amabs,$08,$40
         .byte amzpx,$08,$40
         .byte amabx,$08,$40
gr11     .byte amzp,$00,$30
         .byte amabs,$08,$40
         .byte amzpy,$08,$40
gr12     .byte amzp,$08,$30
         .byte amabs,$08,$40
         .byte amzpy,$08,$40

toktomnm
         .byte $00,<mnbrk,>mnbrk
         .byte $01,<mnora,>mnora
         .byte $06,<mnasl,>mnasl
         .byte $08,<mnphp,>mnphp
         .byte $10,<mnbpl,>mnbpl
         .byte $18,<mnclc,>mnclc
         .byte $20,<mnjsr,>mnjsr
         .byte $21,<mnand,>mnand
         .byte $24,<mnbit,>mnbit
         .byte $26,<mnrol,>mnrol
         .byte $30,<mnbmi,>mnbmi
         .byte $38,<mnsec,>mnsec
         .byte $40,<mnrti,>mnrti
         .byte $41,<mneor,>mneor
         .byte $46,<mnlsr,>mnlsr
         .byte $4c,<mnjmp,>mnjmp
         .byte $50,<mnbvc,>mnbvc
         .byte $58,<mncli,>mncli
         .byte $60,<mnrts,>mnrts
         .byte $61,<mnadc,>mnadc
         .byte $66,<mnror,>mnror
         .byte $70,<mnbvs,>mnbvs
         .byte $78,<mnsei,>mnsei
         .byte $81,<mnsta,>mnsta
         .byte $84,<mnsty,>mnsty
         .byte $86,<mnstx,>mnstx
         .byte $88,<mndey,>mndey
         .byte $8a,<mntxa,>mntxa
         .byte $90,<mnbcc,>mnbcc
         .byte $9a,<mntxs,>mntxs
         .byte $a0,<mnldy,>mnldy
         .byte $a1,<mnlda,>mnlda
         .byte $a2,<mnldx,>mnldx
         .byte $a8,<mntay,>mntay
         .byte $aa,<mntax,>mntax
         .byte $b0,<mnbcs,>mnbcs
         .byte $b8,<mnclv,>mnclv
         .byte $ba,<mntsx,>mntsx
         .byte $c0,<mncpy,>mncpy
         .byte $c1,<mncmp,>mncmp
         .byte $c6,<mndec,>mndec
         .byte $c8,<mniny,>mniny
         .byte $ca,<mndex,>mndex
         .byte $d0,<mnbne,>mnbne
         .byte $d8,<mncld,>mncld
         .byte $e0,<mncpx,>mncpx
         .byte $e1,<mnsbc,>mnsbc
         .byte $e6,<mninc,>mninc
         .byte $e8,<mninx,>mninx
         .byte $ea,<mnnop,>mnnop
         .byte $f0,<mnbeq,>mnbeq
         .byte $f8,<mnsed,>mnsed
tokend

