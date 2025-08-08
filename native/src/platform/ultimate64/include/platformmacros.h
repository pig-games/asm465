     ; Based on the great work by Shallan at: https://github.com/smnjameson
PLATFORMMACROS :?= false
.if !PLATFORMMACROS
PLATFORMMACROS := true
.include "platformdefs.h"

phq .function
	pha
	phx
	phy
.endfunction

plq .function
	ply
	plx
	pla
.endfunction

BasicUpstart .macro addr
		.byte $09,$20 ; End of command marker (first byte after the 00 terminator)
		.byte $0a,$00 ; 10
		.byte $fe,$02,$30,$00 ; BANK 0
		.byte <end, >end  ; End of command marker (first byte after the 00 terminator)
		.byte $14,$00 ; 20
		.byte $9e  ; SYS
		.text format("%4d", \addr)
		.byte $00
	end
		.byte $00,$00	;End of basic terminators
.endmacro

disableCIAandIRQ .macro
    	lda #$7f
        sta $DC0D 
        sta $DD0D 

        lda #$00
        sta $D01A

        lda #$70
        sta $D640
        nop
.endmacro

disableC65ROM .macro
		lda #$70
		sta $d640
		eom
.endmacro

.namespace dma
RunJob .macro JobPointer
		lda #(\JobPointer >> 16)
		sta $d702
		sta $d704
		lda #>\JobPointer
		sta $d701
		lda #<\JobPointer
		sta $d705
.endmacro

Header .macro SourceBank, DestBank
		.byte $0A ; Request format is F018A
		.byte $80, \SourceBank
		.byte $81, \DestBank
.endmacro

Step .macro SourceStep, SourceStepFractional, DestStep, DestStepFractional
		.if \SourceStepFractional != 0
			.byte $82, \SourceStepFractional
		.endif
		.if \SourceStep != 1
			.byte $83, \SourceStep
		.endif
		.if \DestStepFractional != 0
			.byte $84, \DestStepFractional
		.endif
		.if \DestStep != 1
			.byte $85, \DestStep
		.endif
.endmacro

DisableTransparency .macro
		.byte $06
.endmacro

EnableTransparency .macro TransparentByte
		.byte $07 
		.byte $86, \TransparentByte
.endmacro

CopyJob .macro Source, Destination, Length, Chain, Backwards
	.byte $00 ; No more options
	.if \Chain
		.byte $04 ; Copy and chain
	.else
		.byte $00 ; Copy and last request
	.endif
	
	backByte .var 0
	.if \Backwards
		.eval backByte = $40
		.eval \Source = \Source + \Length - 1
		.eval \Destination = \Destination + \Length - 1
	.endif
	.word \Length ; Size of Copy

	.word \Source & $ff
	.byte (\Source >> 16) + backByte

	.word \Destination & $ffff
	.byte ((\Destination >> 16) & $0f) + backByte
	.if \Chain
		.word $0000
	.endif
.endmacro

FillJob .macro SourceByte, Destination, Length, Chain
	.byte $0a ; 11 byte mode
	.byte $81, (\Destination>>20) & $ff ; dest bank
	.byte $00 ; EOL
	.if \Chain
        .byte dma.FILL|dma.CHAIN        ; fill, chain next job
	.else
		.byte dma.FILL ; Fill and last request
	.endif
	.word \Length ; Size of Copy
	.word \SourceByte & $ff
	.byte $00
	.word \Destination & $ffff
	.byte (\Destination >> 16) & $f
	.word $0000
.endmacro


MixJob .macro Source, Destination, Length, Chain, Backwards
	.byte $00 ; No more options
	.if \Chain
		.byte $04 ; Mix and chain
	.else
		.byte $00 ; Mix and last request
	.endif	
	
	backByte .var 0
	.if \Backwards
		.eval backByte = $40
		.eval \Source = \Source + \Length - 1
		.eval \Destination = \Destination + \Length - 1
	.endif
	.word \Length ; Size of Copy
	.word \Source & $ffff
	.byte (\Source >> 16) + backByte
	.word \Destination & $ffff
	.byte ((\Destination >> 16) & $0f) + backByte
	.if \Chain
		.word $0000
	.endif
.endmacro

.endnamespace ; dma

.endif