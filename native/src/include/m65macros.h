     ; Based on the great work by Shallan at: https://github.com/smnjameson

.cpu "4510"

stabpqz  .function address ; sta [BP4],z
        nop
        sta (address),z
    .endfunction

ldqa .function address ; ldq bp/addr/(BP),z/[BP4],z
	neg
	neg
	lda address
.endfunction

stqa .function address ; stq bp/addr/(BP)/[BP4]
	neg
	neg
	sta address
.endfunction

adqa .function address ; adcq bp/addr/(BP)/[BP4]
	neg
	neg
	adc address
.endfunction

phq .function
	pha
	phx
	phy
	phz
.endfunction

plq .function
	plz
	ply
	plx
	pla
.endfunction

BasicUpstart65 .macro addr
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

enable40Mhz .macro
		lda #$41
		sta $00 ; 40 Mhz mode
.endmacro

enableVIC3Registers .macro
		lda #$00
		tax 
		tay 
		taz 
		map
		eom

		lda #$A5	;Enable VIC III
		sta vic4.KEY
		lda #$96
		sta vic4.KEY
.endmacro

enableVIC4Registers .macro
		lda #$00
		tax 
		tay 
		taz 
		map
		eom

		lda #$47	;Enable VIC IV
		sta vic4.KEY
		lda #$53
		sta vic4.KEY
		eom
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

mapMemory .macro source, target
	sourceMB 	 .var (\source & $ff00000) >> 20
	sourceOffset .var ((\source & $00fff00) - target)
	sourceOffset .var ((\source & $00fff00) - target)
	sourceOffHi	 .var \sourceOffset >> 16
	sourceOffLo  .var (\sourceOffset & $0ff00 ) >> 8
	bitLo 		 .var pow(2, (((\target) & $ff00) >> 12) / 2) << 4
	bitHi 	 	 .var pow(2, (((\target-$8000) & $ff00) >> 12) / 2) << 4
	
	.if \target<$8000
		lda #sourceMB
		ldx #$0f
		ldy #$00
		ldz #$00
	.else
		lda #$00
		ldx #$00
		ldy #sourceMB
		ldz #$0f
	.endif
	map 

	; Set offset map
	.if \target<$8000
		lda #sourceOffLo
		ldx #(sourceOffHi + bitLo)
		ldy #$00
		ldz #$00
	.else
		lda #$00
		ldx #$00
		ldy #sourceOffLo
		ldz #(sourceOffHi + bitHi)
	.endif	
	map 
	eom
.endmacro

.namespace vic4
SetCharLocation .macro addr
	lda #(\addr & $ff)
	sta $d068
	lda #((\addr & $ff00)>>8)
	sta $d069
	lda #((\addr & $ff0000)>>16)
	sta $d06a
.endmacro

SetScreenLocation .macro addr
	lda #(\addr & $ff)
	sta $d060
	lda #(\addr & $ff00)>>8)
	sta $d061
	lda #((\addr & $ff0000)>>16)
	sta $d062
	lda #(((\addr & $ff0000)>>24) & $0f)
	sta $d063
.endmacro
.endnamespace

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