;---------------------------------------
; prsrconst.s
;---------------------------------------

prsla    = $01  ; a
prsua    = $41  ; A

prslbr   = $1b  ; [
prsrbr   = $1d  ; ]
prsfsl   = $2f   ; /


;---------------------------------------
; Address mode constants
;---------------------------------------

amimp    = 0
amacc    = 1
amimm    = 2
amzp     = 3
amzpx    = 4
amzpy    = 5
amabs    = 6
amabx    = 7
amaby    = 8
amrel    = 9
amxin    = 10
aminy    = 11
amind    = 12
ambpr    = 13

;---------------------------------------
; LineType constants
;---------------------------------------

ltempty  = %00000000
ltlbdef  = %00000001
ltinstr  = %00000010
ltlbuse  = %00000100
ltdirtv  = %00001000
ltmcdef  = %00010000
ltmcuse  = %00100000
ltcomm   = %01000000
ltrelad  = %10000000

;---------------------------------------
; ValueSpec constants
;---------------------------------------

vshex    = 1*16
vsdec    = 2*16
vsbin    = 3*16
vsoct    = 4*16
vsexpr   = 5*16
vslabel  = 6*16
vschar   = 7*16

