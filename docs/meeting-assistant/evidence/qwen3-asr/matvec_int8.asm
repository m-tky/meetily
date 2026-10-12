
/home/user/Code/meeting-assistant-research/runtime/qwen3-asr/native-lp64-target/release/qwen-native-bench:     file format elf64-x86-64


Disassembly of section .text:

0000000000077900 <qwen_asr::kernels::avx::matvec_int8>:
   77900:	55                   	push   %rbp
   77901:	41 57                	push   %r15
   77903:	41 56                	push   %r14
   77905:	41 55                	push   %r13
   77907:	41 54                	push   %r12
   77909:	53                   	push   %rbx
   7790a:	48 83 ec 28          	sub    $0x28,%rsp
   7790e:	4c 8b 7c 24 78       	mov    0x78(%rsp),%r15
   77913:	4c 8b 6c 24 70       	mov    0x70(%rsp),%r13
   77918:	4c 8b 74 24 68       	mov    0x68(%rsp),%r14
   7791d:	4c 89 0c 24          	mov    %r9,(%rsp)
   77921:	4c 89 44 24 08       	mov    %r8,0x8(%rsp)
   77926:	49 89 f1             	mov    %rsi,%r9
   77929:	48 89 4c 24 10       	mov    %rcx,0x10(%rsp)
   7792e:	49 83 ff 02          	cmp    $0x2,%r15
   77932:	0f 82 e0 03 00 00    	jb     77d18 <qwen_asr::kernels::avx::matvec_int8+0x418>
   77938:	4b 8d 44 2d 00       	lea    0x0(%r13,%r13,1),%rax
   7793d:	4e 8d 64 29 20       	lea    0x20(%rcx,%r13,1),%r12
   77942:	c4 e2 79 18 c8       	vbroadcastss %xmm0,%xmm1
   77947:	48 8d 69 20          	lea    0x20(%rcx),%rbp
   7794b:	45 31 d2             	xor    %r10d,%r10d
   7794e:	4c 89 4c 24 18       	mov    %r9,0x18(%rsp)
   77953:	48 89 44 24 20       	mov    %rax,0x20(%rsp)
   77958:	4a 8d 04 29          	lea    (%rcx,%r13,1),%rax
   7795c:	0f 1f 40 00          	nopl   0x0(%rax)
   77960:	49 83 fd 40          	cmp    $0x40,%r13
   77964:	73 1a                	jae    77980 <qwen_asr::kernels::avx::matvec_int8+0x80>
   77966:	c5 e9 ef d2          	vpxor  %xmm2,%xmm2,%xmm2
   7796a:	c5 e1 ef db          	vpxor  %xmm3,%xmm3,%xmm3
   7796e:	c5 d9 ef e4          	vpxor  %xmm4,%xmm4,%xmm4
   77972:	c5 d1 ef ed          	vpxor  %xmm5,%xmm5,%xmm5
   77976:	31 db                	xor    %ebx,%ebx
   77978:	e9 cc 00 00 00       	jmp    77a49 <qwen_asr::kernels::avx::matvec_int8+0x149>
   7797d:	0f 1f 00             	nopl   (%rax)
   77980:	c5 d1 ef ed          	vpxor  %xmm5,%xmm5,%xmm5
   77984:	45 31 db             	xor    %r11d,%r11d
   77987:	c5 d9 ef e4          	vpxor  %xmm4,%xmm4,%xmm4
   7798b:	c5 e1 ef db          	vpxor  %xmm3,%xmm3,%xmm3
   7798f:	c5 e9 ef d2          	vpxor  %xmm2,%xmm2,%xmm2
   77993:	66 66 66 66 2e 0f 1f 	data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
   7799a:	84 00 00 00 00 00
   779a0:	c4 a2 7d 20 34 1a    	vpmovsxbw (%rdx,%r11,1),%ymm6
   779a6:	c4 a2 7d 20 7c 1d e0 	vpmovsxbw -0x20(%rbp,%r11,1),%ymm7
   779ad:	c4 02 7d 20 5c 1c e0 	vpmovsxbw -0x20(%r12,%r11,1),%ymm11
   779b4:	c4 22 7d 20 74 1a 10 	vpmovsxbw 0x10(%rdx,%r11,1),%ymm14
   779bb:	c4 22 7d 20 6c 1a 20 	vpmovsxbw 0x20(%rdx,%r11,1),%ymm13
   779c2:	c4 22 7d 20 4c 1d 00 	vpmovsxbw 0x0(%rbp,%r11,1),%ymm9
   779c9:	c4 22 7d 20 44 1d f0 	vpmovsxbw -0x10(%rbp,%r11,1),%ymm8
   779d0:	c4 22 7d 20 64 1a 30 	vpmovsxbw 0x30(%rdx,%r11,1),%ymm12
   779d7:	c4 22 7d 20 54 1d 10 	vpmovsxbw 0x10(%rbp,%r11,1),%ymm10
   779de:	49 8d 5b 40          	lea    0x40(%r11),%rbx
   779e2:	c5 cd f5 ff          	vpmaddwd %ymm7,%ymm6,%ymm7
   779e6:	c5 a5 f5 f6          	vpmaddwd %ymm6,%ymm11,%ymm6
   779ea:	c4 41 15 f5 c9       	vpmaddwd %ymm9,%ymm13,%ymm9
   779ef:	c4 41 0d f5 c0       	vpmaddwd %ymm8,%ymm14,%ymm8
   779f4:	c4 41 1d f5 d2       	vpmaddwd %ymm10,%ymm12,%ymm10
   779f9:	c5 cd fe db          	vpaddd %ymm3,%ymm6,%ymm3
   779fd:	c4 82 7d 20 74 1c f0 	vpmovsxbw -0x10(%r12,%r11,1),%ymm6
   77a04:	c5 c5 fe ed          	vpaddd %ymm5,%ymm7,%ymm5
   77a08:	c5 b5 fe e4          	vpaddd %ymm4,%ymm9,%ymm4
   77a0c:	c5 bd fe ed          	vpaddd %ymm5,%ymm8,%ymm5
   77a10:	c5 ad fe e4          	vpaddd %ymm4,%ymm10,%ymm4
   77a14:	c5 8d f5 f6          	vpmaddwd %ymm6,%ymm14,%ymm6
   77a18:	c5 e5 fe de          	vpaddd %ymm6,%ymm3,%ymm3
   77a1c:	c4 82 7d 20 34 1c    	vpmovsxbw (%r12,%r11,1),%ymm6
   77a22:	c5 95 f5 f6          	vpmaddwd %ymm6,%ymm13,%ymm6
   77a26:	c5 cd fe d2          	vpaddd %ymm2,%ymm6,%ymm2
   77a2a:	c4 82 7d 20 74 1c 10 	vpmovsxbw 0x10(%r12,%r11,1),%ymm6
   77a31:	49 83 eb 80          	sub    $0xffffffffffffff80,%r11
   77a35:	c5 9d f5 f6          	vpmaddwd %ymm6,%ymm12,%ymm6
   77a39:	c5 ed fe d6          	vpaddd %ymm6,%ymm2,%ymm2
   77a3d:	4d 39 eb             	cmp    %r13,%r11
   77a40:	49 89 db             	mov    %rbx,%r11
   77a43:	0f 86 57 ff ff ff    	jbe    779a0 <qwen_asr::kernels::avx::matvec_int8+0xa0>
   77a49:	49 89 db             	mov    %rbx,%r11
   77a4c:	49 83 cb 20          	or     $0x20,%r11
   77a50:	4d 39 eb             	cmp    %r13,%r11
   77a53:	77 63                	ja     77ab8 <qwen_asr::kernels::avx::matvec_int8+0x1b8>
   77a55:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
   77a5c:	00 00 00 00
   77a60:	c4 e2 7d 20 34 1a    	vpmovsxbw (%rdx,%rbx,1),%ymm6
   77a66:	c4 e2 7d 20 3c 19    	vpmovsxbw (%rcx,%rbx,1),%ymm7
   77a6c:	c4 62 7d 20 0c 18    	vpmovsxbw (%rax,%rbx,1),%ymm9
   77a72:	c4 62 7d 20 54 1a 10 	vpmovsxbw 0x10(%rdx,%rbx,1),%ymm10
   77a79:	c4 62 7d 20 44 19 10 	vpmovsxbw 0x10(%rcx,%rbx,1),%ymm8
   77a80:	4c 8d 5b 20          	lea    0x20(%rbx),%r11
   77a84:	c5 cd f5 ff          	vpmaddwd %ymm7,%ymm6,%ymm7
   77a88:	c5 b5 f5 f6          	vpmaddwd %ymm6,%ymm9,%ymm6
   77a8c:	c4 41 2d f5 c0       	vpmaddwd %ymm8,%ymm10,%ymm8
   77a91:	c5 cd fe db          	vpaddd %ymm3,%ymm6,%ymm3
   77a95:	c4 e2 7d 20 74 18 10 	vpmovsxbw 0x10(%rax,%rbx,1),%ymm6
   77a9c:	c5 c5 fe ed          	vpaddd %ymm5,%ymm7,%ymm5
   77aa0:	48 83 c3 40          	add    $0x40,%rbx
   77aa4:	c5 bd fe ed          	vpaddd %ymm5,%ymm8,%ymm5
   77aa8:	c5 ad f5 f6          	vpmaddwd %ymm6,%ymm10,%ymm6
   77aac:	c5 e5 fe de          	vpaddd %ymm6,%ymm3,%ymm3
   77ab0:	4c 39 eb             	cmp    %r13,%rbx
   77ab3:	4c 89 db             	mov    %r11,%rbx
   77ab6:	76 a8                	jbe    77a60 <qwen_asr::kernels::avx::matvec_int8+0x160>
   77ab8:	48 8b 34 24          	mov    (%rsp),%rsi
   77abc:	49 39 f2             	cmp    %rsi,%r10
   77abf:	0f 83 a7 05 00 00    	jae    7806c <qwen_asr::kernels::avx::matvec_int8+0x76c>
   77ac5:	4d 89 d3             	mov    %r10,%r11
   77ac8:	49 83 cb 01          	or     $0x1,%r11
   77acc:	49 39 f3             	cmp    %rsi,%r11
   77acf:	0f 83 80 05 00 00    	jae    78055 <qwen_asr::kernels::avx::matvec_int8+0x755>
   77ad5:	c5 d5 fe e4          	vpaddd %ymm4,%ymm5,%ymm4
   77ad9:	c5 e5 fe d2          	vpaddd %ymm2,%ymm3,%ymm2
   77add:	48 8b 74 24 08       	mov    0x8(%rsp),%rsi
   77ae2:	49 89 df             	mov    %rbx,%r15
   77ae5:	4d 29 ef             	sub    %r13,%r15
   77ae8:	c4 e3 7d 39 e5 01    	vextracti128 $0x1,%ymm4,%xmm5
   77aee:	c4 e3 7d 39 d3 01    	vextracti128 $0x1,%ymm2,%xmm3
   77af4:	c5 d1 fe e4          	vpaddd %xmm4,%xmm5,%xmm4
   77af8:	c5 e1 fe d2          	vpaddd %xmm2,%xmm3,%xmm2
   77afc:	c5 f9 70 ec 4e       	vpshufd $0x4e,%xmm4,%xmm5
   77b01:	c5 f9 70 da 4e       	vpshufd $0x4e,%xmm2,%xmm3
   77b06:	c5 d1 fe e4          	vpaddd %xmm4,%xmm5,%xmm4
   77b0a:	c5 e1 fe d2          	vpaddd %xmm2,%xmm3,%xmm2
   77b0e:	c4 e2 59 02 d2       	vphaddd %xmm2,%xmm4,%xmm2
   77b13:	c5 f9 70 d2 e8       	vpshufd $0xe8,%xmm2,%xmm2
   77b18:	c5 f8 5b d2          	vcvtdq2ps %xmm2,%xmm2
   77b1c:	c5 f0 59 da          	vmulps %xmm2,%xmm1,%xmm3
   77b20:	c4 a1 7b 10 14 96    	vmovsd (%rsi,%r10,4),%xmm2
   77b26:	c5 e0 59 da          	vmulps %xmm2,%xmm3,%xmm3
   77b2a:	0f 83 68 01 00 00    	jae    77c98 <qwen_asr::kernels::avx::matvec_int8+0x398>
   77b30:	45 89 ee             	mov    %r13d,%r14d
   77b33:	41 29 de             	sub    %ebx,%r14d
   77b36:	41 83 e6 03          	and    $0x3,%r14d
   77b3a:	74 43                	je     77b7f <qwen_asr::kernels::avx::matvec_int8+0x27f>
   77b3c:	0f 1f 40 00          	nopl   0x0(%rax)
   77b40:	0f b6 34 19          	movzbl (%rcx,%rbx,1),%esi
   77b44:	44 0f be 04 1a       	movsbl (%rdx,%rbx,1),%r8d
   77b49:	c5 f9 6e ee          	vmovd  %esi,%xmm5
   77b4d:	c4 e3 51 20 2c 18 01 	vpinsrb $0x1,(%rax,%rbx,1),%xmm5,%xmm5
   77b54:	c4 c1 02 2a e0       	vcvtsi2ss %r8d,%xmm15,%xmm4
   77b59:	48 ff c3             	inc    %rbx
   77b5c:	49 ff ce             	dec    %r14
   77b5f:	c4 e2 79 18 e4       	vbroadcastss %xmm4,%xmm4
   77b64:	c4 e2 79 21 ed       	vpmovsxbd %xmm5,%xmm5
   77b69:	c5 f8 5b ed          	vcvtdq2ps %xmm5,%xmm5
   77b6d:	c5 d8 59 e5          	vmulps %xmm5,%xmm4,%xmm4
   77b71:	c5 f0 59 e4          	vmulps %xmm4,%xmm1,%xmm4
   77b75:	c5 e8 59 e4          	vmulps %xmm4,%xmm2,%xmm4
   77b79:	c5 e0 58 dc          	vaddps %xmm4,%xmm3,%xmm3
   77b7d:	75 c1                	jne    77b40 <qwen_asr::kernels::avx::matvec_int8+0x240>
   77b7f:	4c 8b 4c 24 18       	mov    0x18(%rsp),%r9
   77b84:	4c 8b 74 24 68       	mov    0x68(%rsp),%r14
   77b89:	49 83 ff fc          	cmp    $0xfffffffffffffffc,%r15
   77b8d:	0f 87 05 01 00 00    	ja     77c98 <qwen_asr::kernels::avx::matvec_int8+0x398>
   77b93:	66 66 66 66 2e 0f 1f 	data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
   77b9a:	84 00 00 00 00 00
   77ba0:	44 0f be 04 1a       	movsbl (%rdx,%rbx,1),%r8d
   77ba5:	0f b6 74 19 03       	movzbl 0x3(%rcx,%rbx,1),%esi
   77baa:	c4 c1 02 2a e0       	vcvtsi2ss %r8d,%xmm15,%xmm4
   77baf:	44 0f b6 04 19       	movzbl (%rcx,%rbx,1),%r8d
   77bb4:	c4 e2 79 18 e4       	vbroadcastss %xmm4,%xmm4
   77bb9:	c4 c1 79 6e e8       	vmovd  %r8d,%xmm5
   77bbe:	c4 e3 51 20 2c 18 01 	vpinsrb $0x1,(%rax,%rbx,1),%xmm5,%xmm5
   77bc5:	44 0f be 44 1a 01    	movsbl 0x1(%rdx,%rbx,1),%r8d
   77bcb:	c4 e2 79 21 ed       	vpmovsxbd %xmm5,%xmm5
   77bd0:	c5 f8 5b ed          	vcvtdq2ps %xmm5,%xmm5
   77bd4:	c5 d8 59 e5          	vmulps %xmm5,%xmm4,%xmm4
   77bd8:	c5 f0 59 e4          	vmulps %xmm4,%xmm1,%xmm4
   77bdc:	c5 e8 59 e4          	vmulps %xmm4,%xmm2,%xmm4
   77be0:	c5 e0 58 dc          	vaddps %xmm4,%xmm3,%xmm3
   77be4:	c4 c1 02 2a e0       	vcvtsi2ss %r8d,%xmm15,%xmm4
   77be9:	44 0f b6 44 19 01    	movzbl 0x1(%rcx,%rbx,1),%r8d
   77bef:	c4 e2 79 18 e4       	vbroadcastss %xmm4,%xmm4
   77bf4:	c4 c1 79 6e e8       	vmovd  %r8d,%xmm5
   77bf9:	c4 e3 51 20 6c 18 01 	vpinsrb $0x1,0x1(%rax,%rbx,1),%xmm5,%xmm5
   77c00:	01
   77c01:	44 0f be 44 1a 02    	movsbl 0x2(%rdx,%rbx,1),%r8d
   77c07:	c4 e2 79 21 ed       	vpmovsxbd %xmm5,%xmm5
   77c0c:	c5 f8 5b ed          	vcvtdq2ps %xmm5,%xmm5
   77c10:	c5 d8 59 e5          	vmulps %xmm5,%xmm4,%xmm4
   77c14:	c5 f0 59 e4          	vmulps %xmm4,%xmm1,%xmm4
   77c18:	c5 e8 59 e4          	vmulps %xmm4,%xmm2,%xmm4
   77c1c:	c5 e0 58 dc          	vaddps %xmm4,%xmm3,%xmm3
   77c20:	c4 c1 02 2a e0       	vcvtsi2ss %r8d,%xmm15,%xmm4
   77c25:	44 0f b6 44 19 02    	movzbl 0x2(%rcx,%rbx,1),%r8d
   77c2b:	c4 e2 79 18 e4       	vbroadcastss %xmm4,%xmm4
   77c30:	c4 c1 79 6e e8       	vmovd  %r8d,%xmm5
   77c35:	c4 e3 51 20 6c 18 02 	vpinsrb $0x1,0x2(%rax,%rbx,1),%xmm5,%xmm5
   77c3c:	01
   77c3d:	44 0f be 44 1a 03    	movsbl 0x3(%rdx,%rbx,1),%r8d
   77c43:	c4 e2 79 21 ed       	vpmovsxbd %xmm5,%xmm5
   77c48:	c5 f8 5b ed          	vcvtdq2ps %xmm5,%xmm5
   77c4c:	c5 d8 59 e5          	vmulps %xmm5,%xmm4,%xmm4
   77c50:	c5 f9 6e ee          	vmovd  %esi,%xmm5
   77c54:	c4 e3 51 20 6c 18 03 	vpinsrb $0x1,0x3(%rax,%rbx,1),%xmm5,%xmm5
   77c5b:	01
   77c5c:	48 83 c3 04          	add    $0x4,%rbx
   77c60:	c5 f0 59 e4          	vmulps %xmm4,%xmm1,%xmm4
   77c64:	c5 e8 59 e4          	vmulps %xmm4,%xmm2,%xmm4
   77c68:	c4 e2 79 21 ed       	vpmovsxbd %xmm5,%xmm5
   77c6d:	c5 e0 58 dc          	vaddps %xmm4,%xmm3,%xmm3
   77c71:	c4 c1 02 2a e0       	vcvtsi2ss %r8d,%xmm15,%xmm4
   77c76:	c5 f8 5b ed          	vcvtdq2ps %xmm5,%xmm5
   77c7a:	c4 e2 79 18 e4       	vbroadcastss %xmm4,%xmm4
   77c7f:	c5 d8 59 e5          	vmulps %xmm5,%xmm4,%xmm4
   77c83:	c5 f0 59 e4          	vmulps %xmm4,%xmm1,%xmm4
   77c87:	c5 e8 59 e4          	vmulps %xmm4,%xmm2,%xmm4
   77c8b:	c5 e0 58 dc          	vaddps %xmm4,%xmm3,%xmm3
   77c8f:	49 39 dd             	cmp    %rbx,%r13
   77c92:	0f 85 08 ff ff ff    	jne    77ba0 <qwen_asr::kernels::avx::matvec_int8+0x2a0>
   77c98:	48 83 7c 24 60 00    	cmpq   $0x0,0x60(%rsp)
   77c9e:	4c 8b 7c 24 78       	mov    0x78(%rsp),%r15
   77ca3:	74 21                	je     77cc6 <qwen_asr::kernels::avx::matvec_int8+0x3c6>
   77ca5:	4d 39 f2             	cmp    %r14,%r10
   77ca8:	0f 83 3f 04 00 00    	jae    780ed <qwen_asr::kernels::avx::matvec_int8+0x7ed>
   77cae:	4d 39 f3             	cmp    %r14,%r11
   77cb1:	0f 83 20 04 00 00    	jae    780d7 <qwen_asr::kernels::avx::matvec_int8+0x7d7>
   77cb7:	48 8b 74 24 60       	mov    0x60(%rsp),%rsi
   77cbc:	c4 a1 7b 10 14 96    	vmovsd (%rsi,%r10,4),%xmm2
   77cc2:	c5 e0 58 da          	vaddps %xmm2,%xmm3,%xmm3
   77cc6:	4d 39 ca             	cmp    %r9,%r10
   77cc9:	0f 83 f2 03 00 00    	jae    780c1 <qwen_asr::kernels::avx::matvec_int8+0x7c1>
   77ccf:	c4 a1 7a 11 1c 97    	vmovss %xmm3,(%rdi,%r10,4)
   77cd5:	4d 39 cb             	cmp    %r9,%r11
   77cd8:	0f 83 cd 03 00 00    	jae    780ab <qwen_asr::kernels::avx::matvec_int8+0x7ab>
   77cde:	48 8b 74 24 20       	mov    0x20(%rsp),%rsi
   77ce3:	c4 a3 79 17 1c 9f 01 	vextractps $0x1,%xmm3,(%rdi,%r11,4)
   77cea:	4d 8d 5a 02          	lea    0x2(%r10),%r11
   77cee:	49 83 c2 03          	add    $0x3,%r10
   77cf2:	49 01 f4             	add    %rsi,%r12
   77cf5:	48 01 f5             	add    %rsi,%rbp
   77cf8:	48 01 f0             	add    %rsi,%rax
   77cfb:	48 01 f1             	add    %rsi,%rcx
   77cfe:	4d 39 fa             	cmp    %r15,%r10
   77d01:	4d 89 da             	mov    %r11,%r10
   77d04:	0f 82 56 fc ff ff    	jb     77960 <qwen_asr::kernels::avx::matvec_int8+0x60>
   77d0a:	4c 8b 24 24          	mov    (%rsp),%r12
   77d0e:	4d 39 fb             	cmp    %r15,%r11
   77d11:	72 15                	jb     77d28 <qwen_asr::kernels::avx::matvec_int8+0x428>
   77d13:	e9 2b 03 00 00       	jmp    78043 <qwen_asr::kernels::avx::matvec_int8+0x743>
   77d18:	45 31 db             	xor    %r11d,%r11d
   77d1b:	4c 8b 24 24          	mov    (%rsp),%r12
   77d1f:	4d 39 fb             	cmp    %r15,%r11
   77d22:	0f 83 1b 03 00 00    	jae    78043 <qwen_asr::kernels::avx::matvec_int8+0x743>
   77d28:	48 8b 74 24 10       	mov    0x10(%rsp),%rsi
   77d2d:	4c 89 d8             	mov    %r11,%rax
   77d30:	49 0f af c5          	imul   %r13,%rax
   77d34:	48 8d 4c 06 20       	lea    0x20(%rsi,%rax,1),%rcx
   77d39:	48 01 f0             	add    %rsi,%rax
   77d3c:	0f 1f 40 00          	nopl   0x0(%rax)
   77d40:	49 83 fd 40          	cmp    $0x40,%r13
   77d44:	73 1a                	jae    77d60 <qwen_asr::kernels::avx::matvec_int8+0x460>
   77d46:	c5 f0 57 c9          	vxorps %xmm1,%xmm1,%xmm1
   77d4a:	c5 e8 57 d2          	vxorps %xmm2,%xmm2,%xmm2
   77d4e:	45 31 d2             	xor    %r10d,%r10d
   77d51:	e9 80 00 00 00       	jmp    77dd6 <qwen_asr::kernels::avx::matvec_int8+0x4d6>
   77d56:	66 2e 0f 1f 84 00 00 	cs nopw 0x0(%rax,%rax,1)
   77d5d:	00 00 00
   77d60:	c5 e8 57 d2          	vxorps %xmm2,%xmm2,%xmm2
   77d64:	45 31 c0             	xor    %r8d,%r8d
   77d67:	c5 f0 57 c9          	vxorps %xmm1,%xmm1,%xmm1
   77d6b:	0f 1f 44 00 00       	nopl   0x0(%rax,%rax,1)
   77d70:	c4 a2 7d 20 1c 02    	vpmovsxbw (%rdx,%r8,1),%ymm3
   77d76:	c4 a2 7d 20 64 01 e0 	vpmovsxbw -0x20(%rcx,%r8,1),%ymm4
   77d7d:	4d 8d 50 40          	lea    0x40(%r8),%r10
   77d81:	c5 e5 f5 dc          	vpmaddwd %ymm4,%ymm3,%ymm3
   77d85:	c4 a2 7d 20 64 01 f0 	vpmovsxbw -0x10(%rcx,%r8,1),%ymm4
   77d8c:	c5 e5 fe d2          	vpaddd %ymm2,%ymm3,%ymm2
   77d90:	c4 a2 7d 20 5c 02 10 	vpmovsxbw 0x10(%rdx,%r8,1),%ymm3
   77d97:	c5 e5 f5 dc          	vpmaddwd %ymm4,%ymm3,%ymm3
   77d9b:	c4 a2 7d 20 24 01    	vpmovsxbw (%rcx,%r8,1),%ymm4
   77da1:	c5 ed fe d3          	vpaddd %ymm3,%ymm2,%ymm2
   77da5:	c4 a2 7d 20 5c 02 20 	vpmovsxbw 0x20(%rdx,%r8,1),%ymm3
   77dac:	c5 e5 f5 dc          	vpmaddwd %ymm4,%ymm3,%ymm3
   77db0:	c4 a2 7d 20 64 01 10 	vpmovsxbw 0x10(%rcx,%r8,1),%ymm4
   77db7:	c5 e5 fe c9          	vpaddd %ymm1,%ymm3,%ymm1
   77dbb:	c4 a2 7d 20 5c 02 30 	vpmovsxbw 0x30(%rdx,%r8,1),%ymm3
   77dc2:	49 83 e8 80          	sub    $0xffffffffffffff80,%r8
   77dc6:	c5 e5 f5 dc          	vpmaddwd %ymm4,%ymm3,%ymm3
   77dca:	c5 f5 fe cb          	vpaddd %ymm3,%ymm1,%ymm1
   77dce:	4d 39 e8             	cmp    %r13,%r8
   77dd1:	4d 89 d0             	mov    %r10,%r8
   77dd4:	76 9a                	jbe    77d70 <qwen_asr::kernels::avx::matvec_int8+0x470>
   77dd6:	4d 89 d0             	mov    %r10,%r8
   77dd9:	49 83 c8 20          	or     $0x20,%r8
   77ddd:	4d 39 e8             	cmp    %r13,%r8
   77de0:	77 48                	ja     77e2a <qwen_asr::kernels::avx::matvec_int8+0x52a>
   77de2:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
   77de9:	1f 84 00 00 00 00 00
   77df0:	c4 a2 7d 20 1c 12    	vpmovsxbw (%rdx,%r10,1),%ymm3
   77df6:	c4 a2 7d 20 24 10    	vpmovsxbw (%rax,%r10,1),%ymm4
   77dfc:	4d 8d 42 20          	lea    0x20(%r10),%r8
   77e00:	c5 e5 f5 dc          	vpmaddwd %ymm4,%ymm3,%ymm3
   77e04:	c4 a2 7d 20 64 10 10 	vpmovsxbw 0x10(%rax,%r10,1),%ymm4
   77e0b:	c5 e5 fe d2          	vpaddd %ymm2,%ymm3,%ymm2
   77e0f:	c4 a2 7d 20 5c 12 10 	vpmovsxbw 0x10(%rdx,%r10,1),%ymm3
   77e16:	49 83 c2 40          	add    $0x40,%r10
   77e1a:	c5 e5 f5 dc          	vpmaddwd %ymm4,%ymm3,%ymm3
   77e1e:	c5 ed fe d3          	vpaddd %ymm3,%ymm2,%ymm2
   77e22:	4d 39 ea             	cmp    %r13,%r10
   77e25:	4d 89 c2             	mov    %r8,%r10
   77e28:	76 c6                	jbe    77df0 <qwen_asr::kernels::avx::matvec_int8+0x4f0>
   77e2a:	4d 39 e3             	cmp    %r12,%r11
   77e2d:	0f 83 4c 02 00 00    	jae    7807f <qwen_asr::kernels::avx::matvec_int8+0x77f>
   77e33:	c5 ed fe c9          	vpaddd %ymm1,%ymm2,%ymm1
   77e37:	48 8b 74 24 08       	mov    0x8(%rsp),%rsi
   77e3c:	4c 89 d3             	mov    %r10,%rbx
   77e3f:	4c 29 eb             	sub    %r13,%rbx
   77e42:	c4 e3 7d 39 ca 01    	vextracti128 $0x1,%ymm1,%xmm2
   77e48:	c5 e9 fe c9          	vpaddd %xmm1,%xmm2,%xmm1
   77e4c:	c5 f9 70 d1 4e       	vpshufd $0x4e,%xmm1,%xmm2
   77e51:	c5 e9 fe c9          	vpaddd %xmm1,%xmm2,%xmm1
   77e55:	c5 f9 70 d1 55       	vpshufd $0x55,%xmm1,%xmm2
   77e5a:	c5 e9 fe c9          	vpaddd %xmm1,%xmm2,%xmm1
   77e5e:	c5 f8 5b c9          	vcvtdq2ps %xmm1,%xmm1
   77e62:	c5 fa 59 d1          	vmulss %xmm1,%xmm0,%xmm2
   77e66:	c4 a1 7a 10 0c 9e    	vmovss (%rsi,%r11,4),%xmm1
   77e6c:	c5 ea 59 d1          	vmulss %xmm1,%xmm2,%xmm2
   77e70:	0f 83 94 01 00 00    	jae    7800a <qwen_asr::kernels::avx::matvec_int8+0x70a>
   77e76:	45 89 e8             	mov    %r13d,%r8d
   77e79:	45 29 d0             	sub    %r10d,%r8d
   77e7c:	41 83 e0 07          	and    $0x7,%r8d
   77e80:	74 38                	je     77eba <qwen_asr::kernels::avx::matvec_int8+0x5ba>
   77e82:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
   77e89:	1f 84 00 00 00 00 00
   77e90:	42 0f be 2c 12       	movsbl (%rdx,%r10,1),%ebp
   77e95:	42 0f be 34 10       	movsbl (%rax,%r10,1),%esi
   77e9a:	49 ff c2             	inc    %r10
   77e9d:	49 ff c8             	dec    %r8
   77ea0:	c5 82 2a dd          	vcvtsi2ss %ebp,%xmm15,%xmm3
   77ea4:	c5 82 2a e6          	vcvtsi2ss %esi,%xmm15,%xmm4
   77ea8:	c5 e2 59 dc          	vmulss %xmm4,%xmm3,%xmm3
   77eac:	c5 fa 59 db          	vmulss %xmm3,%xmm0,%xmm3
   77eb0:	c5 f2 59 db          	vmulss %xmm3,%xmm1,%xmm3
   77eb4:	c5 ea 58 d3          	vaddss %xmm3,%xmm2,%xmm2
   77eb8:	75 d6                	jne    77e90 <qwen_asr::kernels::avx::matvec_int8+0x590>
   77eba:	48 83 fb f8          	cmp    $0xfffffffffffffff8,%rbx
   77ebe:	0f 87 46 01 00 00    	ja     7800a <qwen_asr::kernels::avx::matvec_int8+0x70a>
   77ec4:	66 66 66 2e 0f 1f 84 	data16 data16 cs nopw 0x0(%rax,%rax,1)
   77ecb:	00 00 00 00 00
   77ed0:	46 0f be 04 12       	movsbl (%rdx,%r10,1),%r8d
   77ed5:	42 0f be 74 10 07    	movsbl 0x7(%rax,%r10,1),%esi
   77edb:	c4 c1 02 2a d8       	vcvtsi2ss %r8d,%xmm15,%xmm3
   77ee0:	46 0f be 04 10       	movsbl (%rax,%r10,1),%r8d
   77ee5:	c4 c1 02 2a e0       	vcvtsi2ss %r8d,%xmm15,%xmm4
   77eea:	46 0f be 44 12 01    	movsbl 0x1(%rdx,%r10,1),%r8d
   77ef0:	c5 e2 59 dc          	vmulss %xmm4,%xmm3,%xmm3
   77ef4:	c5 fa 59 db          	vmulss %xmm3,%xmm0,%xmm3
   77ef8:	c5 f2 59 db          	vmulss %xmm3,%xmm1,%xmm3
   77efc:	c5 ea 58 d3          	vaddss %xmm3,%xmm2,%xmm2
   77f00:	c4 c1 02 2a d8       	vcvtsi2ss %r8d,%xmm15,%xmm3
   77f05:	46 0f be 44 10 01    	movsbl 0x1(%rax,%r10,1),%r8d
   77f0b:	c4 c1 02 2a e0       	vcvtsi2ss %r8d,%xmm15,%xmm4
   77f10:	46 0f be 44 12 02    	movsbl 0x2(%rdx,%r10,1),%r8d
   77f16:	c5 e2 59 dc          	vmulss %xmm4,%xmm3,%xmm3
   77f1a:	c4 c1 02 2a e0       	vcvtsi2ss %r8d,%xmm15,%xmm4
   77f1f:	46 0f be 44 10 02    	movsbl 0x2(%rax,%r10,1),%r8d
   77f25:	c5 fa 59 db          	vmulss %xmm3,%xmm0,%xmm3
   77f29:	c5 f2 59 db          	vmulss %xmm3,%xmm1,%xmm3
   77f2d:	c5 ea 58 d3          	vaddss %xmm3,%xmm2,%xmm2
   77f31:	c4 c1 02 2a d8       	vcvtsi2ss %r8d,%xmm15,%xmm3
   77f36:	46 0f be 44 12 03    	movsbl 0x3(%rdx,%r10,1),%r8d
   77f3c:	c5 da 59 db          	vmulss %xmm3,%xmm4,%xmm3
   77f40:	c5 fa 59 db          	vmulss %xmm3,%xmm0,%xmm3
   77f44:	c5 f2 59 db          	vmulss %xmm3,%xmm1,%xmm3
   77f48:	c5 ea 58 d3          	vaddss %xmm3,%xmm2,%xmm2
   77f4c:	c4 c1 02 2a d8       	vcvtsi2ss %r8d,%xmm15,%xmm3
   77f51:	46 0f be 44 10 03    	movsbl 0x3(%rax,%r10,1),%r8d
   77f57:	c4 c1 02 2a e0       	vcvtsi2ss %r8d,%xmm15,%xmm4
   77f5c:	46 0f be 44 12 04    	movsbl 0x4(%rdx,%r10,1),%r8d
   77f62:	c5 e2 59 dc          	vmulss %xmm4,%xmm3,%xmm3
   77f66:	c5 fa 59 db          	vmulss %xmm3,%xmm0,%xmm3
   77f6a:	c5 f2 59 db          	vmulss %xmm3,%xmm1,%xmm3
   77f6e:	c5 ea 58 d3          	vaddss %xmm3,%xmm2,%xmm2
   77f72:	c4 c1 02 2a d8       	vcvtsi2ss %r8d,%xmm15,%xmm3
   77f77:	46 0f be 44 10 04    	movsbl 0x4(%rax,%r10,1),%r8d
   77f7d:	c4 c1 02 2a e0       	vcvtsi2ss %r8d,%xmm15,%xmm4
   77f82:	46 0f be 44 12 05    	movsbl 0x5(%rdx,%r10,1),%r8d
   77f88:	c5 e2 59 dc          	vmulss %xmm4,%xmm3,%xmm3
   77f8c:	c4 c1 02 2a e0       	vcvtsi2ss %r8d,%xmm15,%xmm4
   77f91:	46 0f be 44 10 05    	movsbl 0x5(%rax,%r10,1),%r8d
   77f97:	c5 fa 59 db          	vmulss %xmm3,%xmm0,%xmm3
   77f9b:	c5 f2 59 db          	vmulss %xmm3,%xmm1,%xmm3
   77f9f:	c5 ea 58 d3          	vaddss %xmm3,%xmm2,%xmm2
   77fa3:	c4 c1 02 2a d8       	vcvtsi2ss %r8d,%xmm15,%xmm3
   77fa8:	46 0f be 44 12 06    	movsbl 0x6(%rdx,%r10,1),%r8d
   77fae:	c5 da 59 db          	vmulss %xmm3,%xmm4,%xmm3
   77fb2:	c5 fa 59 db          	vmulss %xmm3,%xmm0,%xmm3
   77fb6:	c5 f2 59 db          	vmulss %xmm3,%xmm1,%xmm3
   77fba:	c5 ea 58 d3          	vaddss %xmm3,%xmm2,%xmm2
   77fbe:	c4 c1 02 2a d8       	vcvtsi2ss %r8d,%xmm15,%xmm3
   77fc3:	46 0f be 44 10 06    	movsbl 0x6(%rax,%r10,1),%r8d
   77fc9:	c4 c1 02 2a e0       	vcvtsi2ss %r8d,%xmm15,%xmm4
   77fce:	46 0f be 44 12 07    	movsbl 0x7(%rdx,%r10,1),%r8d
   77fd4:	49 83 c2 08          	add    $0x8,%r10
   77fd8:	c5 e2 59 dc          	vmulss %xmm4,%xmm3,%xmm3
   77fdc:	c5 82 2a e6          	vcvtsi2ss %esi,%xmm15,%xmm4
   77fe0:	c5 fa 59 db          	vmulss %xmm3,%xmm0,%xmm3
   77fe4:	c5 f2 59 db          	vmulss %xmm3,%xmm1,%xmm3
   77fe8:	c5 ea 58 d3          	vaddss %xmm3,%xmm2,%xmm2
   77fec:	c4 c1 02 2a d8       	vcvtsi2ss %r8d,%xmm15,%xmm3
   77ff1:	c5 e2 59 dc          	vmulss %xmm4,%xmm3,%xmm3
   77ff5:	c5 fa 59 db          	vmulss %xmm3,%xmm0,%xmm3
   77ff9:	c5 f2 59 db          	vmulss %xmm3,%xmm1,%xmm3
   77ffd:	c5 ea 58 d3          	vaddss %xmm3,%xmm2,%xmm2
   78001:	4d 39 d5             	cmp    %r10,%r13
   78004:	0f 85 c6 fe ff ff    	jne    77ed0 <qwen_asr::kernels::avx::matvec_int8+0x5d0>
   7800a:	48 83 7c 24 60 00    	cmpq   $0x0,0x60(%rsp)
   78010:	74 14                	je     78026 <qwen_asr::kernels::avx::matvec_int8+0x726>
   78012:	4d 39 f3             	cmp    %r14,%r11
   78015:	0f 83 e8 00 00 00    	jae    78103 <qwen_asr::kernels::avx::matvec_int8+0x803>
   7801b:	48 8b 74 24 60       	mov    0x60(%rsp),%rsi
   78020:	c4 a1 6a 58 14 9e    	vaddss (%rsi,%r11,4),%xmm2,%xmm2
   78026:	4d 39 cb             	cmp    %r9,%r11
   78029:	73 6a                	jae    78095 <qwen_asr::kernels::avx::matvec_int8+0x795>
   7802b:	c4 a1 7a 11 14 9f    	vmovss %xmm2,(%rdi,%r11,4)
   78031:	49 ff c3             	inc    %r11
   78034:	4c 01 e9             	add    %r13,%rcx
   78037:	4c 01 e8             	add    %r13,%rax
   7803a:	4d 39 fb             	cmp    %r15,%r11
   7803d:	0f 85 fd fc ff ff    	jne    77d40 <qwen_asr::kernels::avx::matvec_int8+0x440>
   78043:	48 83 c4 28          	add    $0x28,%rsp
   78047:	5b                   	pop    %rbx
   78048:	41 5c                	pop    %r12
   7804a:	41 5d                	pop    %r13
   7804c:	41 5e                	pop    %r14
   7804e:	41 5f                	pop    %r15
   78050:	5d                   	pop    %rbp
   78051:	c5 f8 77             	vzeroupper
   78054:	c3                   	ret
   78055:	48 8b 34 24          	mov    (%rsp),%rsi
   78059:	48 8d 15 b0 97 07 00 	lea    0x797b0(%rip),%rdx        # f1810 <anon.bf739af1d4689a06542f64c91d721211.3.llvm.3037089127654623880+0x22d8>
   78060:	4c 89 df             	mov    %r11,%rdi
   78063:	c5 f8 77             	vzeroupper
   78066:	ff 15 74 c9 07 00    	call   *0x7c974(%rip)        # f49e0 <_GLOBAL_OFFSET_TABLE_+0x7a8>
   7806c:	48 8d 15 85 97 07 00 	lea    0x79785(%rip),%rdx        # f17f8 <anon.bf739af1d4689a06542f64c91d721211.3.llvm.3037089127654623880+0x22c0>
   78073:	4c 89 d7             	mov    %r10,%rdi
   78076:	c5 f8 77             	vzeroupper
   78079:	ff 15 61 c9 07 00    	call   *0x7c961(%rip)        # f49e0 <_GLOBAL_OFFSET_TABLE_+0x7a8>
   7807f:	48 8d 15 2a 97 07 00 	lea    0x7972a(%rip),%rdx        # f17b0 <anon.bf739af1d4689a06542f64c91d721211.3.llvm.3037089127654623880+0x2278>
   78086:	4c 89 df             	mov    %r11,%rdi
   78089:	4c 89 e6             	mov    %r12,%rsi
   7808c:	c5 f8 77             	vzeroupper
   7808f:	ff 15 4b c9 07 00    	call   *0x7c94b(%rip)        # f49e0 <_GLOBAL_OFFSET_TABLE_+0x7a8>
   78095:	48 8d 15 44 97 07 00 	lea    0x79744(%rip),%rdx        # f17e0 <anon.bf739af1d4689a06542f64c91d721211.3.llvm.3037089127654623880+0x22a8>
   7809c:	4c 89 df             	mov    %r11,%rdi
   7809f:	4c 89 ce             	mov    %r9,%rsi
   780a2:	c5 f8 77             	vzeroupper
   780a5:	ff 15 35 c9 07 00    	call   *0x7c935(%rip)        # f49e0 <_GLOBAL_OFFSET_TABLE_+0x7a8>
   780ab:	48 8d 15 be 97 07 00 	lea    0x797be(%rip),%rdx        # f1870 <anon.bf739af1d4689a06542f64c91d721211.3.llvm.3037089127654623880+0x2338>
   780b2:	4c 89 df             	mov    %r11,%rdi
   780b5:	4c 89 ce             	mov    %r9,%rsi
   780b8:	c5 f8 77             	vzeroupper
   780bb:	ff 15 1f c9 07 00    	call   *0x7c91f(%rip)        # f49e0 <_GLOBAL_OFFSET_TABLE_+0x7a8>
   780c1:	48 8d 15 90 97 07 00 	lea    0x79790(%rip),%rdx        # f1858 <anon.bf739af1d4689a06542f64c91d721211.3.llvm.3037089127654623880+0x2320>
   780c8:	4c 89 d7             	mov    %r10,%rdi
   780cb:	4c 89 ce             	mov    %r9,%rsi
   780ce:	c5 f8 77             	vzeroupper
   780d1:	ff 15 09 c9 07 00    	call   *0x7c909(%rip)        # f49e0 <_GLOBAL_OFFSET_TABLE_+0x7a8>
   780d7:	48 8d 15 62 97 07 00 	lea    0x79762(%rip),%rdx        # f1840 <anon.bf739af1d4689a06542f64c91d721211.3.llvm.3037089127654623880+0x2308>
   780de:	4c 89 df             	mov    %r11,%rdi
   780e1:	4c 89 f6             	mov    %r14,%rsi
   780e4:	c5 f8 77             	vzeroupper
   780e7:	ff 15 f3 c8 07 00    	call   *0x7c8f3(%rip)        # f49e0 <_GLOBAL_OFFSET_TABLE_+0x7a8>
   780ed:	48 8d 15 34 97 07 00 	lea    0x79734(%rip),%rdx        # f1828 <anon.bf739af1d4689a06542f64c91d721211.3.llvm.3037089127654623880+0x22f0>
   780f4:	4c 89 d7             	mov    %r10,%rdi
   780f7:	4c 89 f6             	mov    %r14,%rsi
   780fa:	c5 f8 77             	vzeroupper
   780fd:	ff 15 dd c8 07 00    	call   *0x7c8dd(%rip)        # f49e0 <_GLOBAL_OFFSET_TABLE_+0x7a8>
   78103:	48 8d 15 be 96 07 00 	lea    0x796be(%rip),%rdx        # f17c8 <anon.bf739af1d4689a06542f64c91d721211.3.llvm.3037089127654623880+0x2290>
   7810a:	4c 89 df             	mov    %r11,%rdi
   7810d:	4c 89 f6             	mov    %r14,%rsi
   78110:	c5 f8 77             	vzeroupper
   78113:	ff 15 c7 c8 07 00    	call   *0x7c8c7(%rip)        # f49e0 <_GLOBAL_OFFSET_TABLE_+0x7a8>
   78119:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)
