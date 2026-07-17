# `jump_beef` World, Mark, Sky, And Cubemap VTF Inventory

Source closure: configured `maps/jump_beef.bsp` SHA-256 `b2e22010b56aa03387c76396a55f2fb83cdeb72a9562ed16cfb656a747e58959`; exact 294-entry `PSDB` bundle SHA-256 `34cbd09a63f1ba8407c7a775de20467773f87a41db78e34447734799fa2dba78`.

Each row records `version; top-level dimensions; format code; mip count; frame count; effective flags`. All selected planes use top-to-bottom row order. Codes 3, 12, 13, 15, and 24 are handled as BGR888/RGB-U8, BGRA8888/RGBA-U8, DXT1/RGBA-U8 opaque, DXT5/RGBA-U8 A8, and RGBA16F/RGBA-F16 respectively.

## World textures

| Logical path | SHA-256 | Metadata |
|---|---|---|
| `materials/wood/wood_floor002.vtf` | `e3aaac37a8737dcefc928ddd1d969cc12d27c630a78cbc712332cf77c186790b` | `7.4; 1024×1024; 13; 11; 1; 0x00000040` |
| `materials/overlays/detail006.vtf` | `39551dad31e56770be62c4a4402bafc2d5968e66f682f6a90367c2abf901f2d1` | `7.4; 1024×1024; 13; 11; 1; 0x00000400` |
| `materials/tools/toolsnodraw.vtf` | `d23876f8302a93b77449e9d548aaacbed59a0de0c2fa1f14f0adf3c6e08fe838` | `7.1; 64×64; 13; 7; 1; 0x00000040` |
| `materials/wood/wall020b.vtf` | `f011c2288d8fab0c604a4a61e59a1e9c5000b21e90009fe7b7853aeb8237962e` | `7.3; 1024×1024; 13; 11; 1; 0x00000040` |
| `materials/wood/wall020_height-ssbump.vtf` | `102fb7fc10b0280eb5032533ac5ad3ffe8fbe5a90cfb3bc3895fe50f9fb822dc` | `7.3; 1024×1024; 13; 11; 1; 0x08000000` |
| `materials/wood/wall007a.vtf` | `4ca43a4c0af6a304f619e4ed7a5b93385fc544d1aebcf5f3767e7d9599055e56` | `7.4; 1024×1024; 13; 11; 1; 0x00000000` |
| `materials/wood/wall015b.vtf` | `0e0b4d247eabab4189d1feebb520c7567b3fb19b5acf92c257af9f17260684fe` | `7.3; 1024×1024; 13; 11; 1; 0x00000040` |
| `materials/tools/toolsskybox.vtf` | `d57cd353783debaafb18b07c156e6b502d4c55a0c26243e937a02e6d26c3eaec` | `7.1; 128×128; 13; 8; 1; 0x00000040` |
| `materials/concrete/concretewall047a.vtf` | `6bee4cec2a00a6cc14bb651a26cfe088fa278a2f1299f320cc738cd53ac1a0b3` | `7.1; 512×512; 13; 10; 1; 0x00000040` |
| `materials/detail/noise_detail_01.vtf` | `b9aeea50fd114e95e2b1a19175fa1b492da06acb2411d5a4f02a125b9a4504e8` | `7.4; 256×256; 13; 9; 1; 0x00000400` |
| `materials/tools/toolstrigger.vtf` | `3142dcbb52c7b9b8a95bbc4fbae6f8f9f2511877265c7e55f5e1e0f2983a46d5` | `7.1; 64×64; 15; 7; 1; 0x00002040` |
| `materials/metal/metalfence007a.vtf` | `51431161fb9ad7aeb6d8df5d8f50a16f0005e08ba403bda55cf36b2883ddf293` | `7.3; 512×512; 15; 10; 1; 0x00002040` |
| `materials/detail/metal_detail_01.vtf` | `f711fb26d46ae656b070ad877aef5459a5fbf5ef50894953ef14765de99f29e9` | `7.4; 512×512; 13; 10; 1; 0x00000400` |
| `materials/glass/glasswindow002a.vtf` | `a79d35afaa7643a8f5e672394a03ded73876f716f93c27bd3cf2d3ebfef11970` | `7.3; 512×512; 15; 10; 1; 0x00002040` |
| `materials/water/tfwater001_normal.vtf` | `7b5de49340bfe1ec2f1e37d771289d42773414f130767b5632ca29467494c017` | `7.3; 256×256; 3; 9; 60; 0x00000080` |

## Decal textures

All number textures are `7.3; 128×256; format 15; 9 mips; 1 frame; flags 0x0000204c`. All arrow textures are `7.3; 256×256; format 15; 9 mips; 1 frame; flags 0x0000224c`. Both groups are sRGB, eight-bit alpha, clamp-S, and clamp-T; arrows additionally set no-LOD.

| Logical path | Source SHA-256 | Mip-0 RGBA SHA-256 | Alpha-zero texels |
|---|---|---|---:|
| `materials/signs/number_00.vtf` | `bac3f3851703069543c94e6d42d2caa7ce42fe96645ccb0c5ea0fc37d471b313` | `f722bf6612c3fd9e36f95aadcfc381ff22bbe2a8e51e459efec7c5eec7659f92` | 19,796 |
| `materials/signs/number_01.vtf` | `de1e006ace891068e7abd37d094967db81256fcb6e41ba23e2cffacb16d4fbc1` | `0eaa8d3e6c77f8cc2ab9b548bc7a7af8d0b1a869a61613b49568c421e76dc380` | 368 |
| `materials/signs/number_02.vtf` | `7550b2dcb97fe575ad8defcc6aa6e2d5db7224055a7bcb48f8ad0aa516aeedba` | `498067942c58294235b306766920de887a584d4a048bff8557b076a2c130d28a` | 762 |
| `materials/signs/number_03.vtf` | `34c774532c83e123699839239d58772163ffef1fc52b2d8962455e4092179eb4` | `ca70f08b111c6c78b54a10efbbd58bb4b54ccd812f6020a6ae61476f0ac18f02` | 843 |
| `materials/signs/number_04.vtf` | `0b1abe48cf9e1b8fa16956a7ee5642dff55cab62aea2488829357954c286619d` | `cf0a7f0c0090d6e561e4d70f3f8e5f0d3c914782e39768e4c0778973af829c0c` | 21,549 |
| `materials/signs/number_05.vtf` | `aad9bdb0c1231d0af04fa04234edc0e4fa9b1a9bda115a6fbbfa2ffff58e10da` | `84ef01682b7bb65d919e182093339d74023fadd9f651dc9ff75cc10285ae355d` | 20,561 |
| `materials/signs/number_06.vtf` | `3495b5532b11d5a3ac7ac673efd74880f328416ba5c3cb71381f9ec3d4791802` | `3d8e1b8ac2c036467b668a286842d6fc0f6ce9c62b287cc2a05f4ff028e8722a` | 19,394 |
| `materials/signs/number_07.vtf` | `c989fec514ea396ef1a221adab3dea9d85ee823aff82a79e000ca0cabb0caefd` | `d6e0d86f75c429b11070023bcdbc02e05cbea3c5f214e6f9051eb6ccabb489ba` | 24,113 |
| `materials/signs/number_08.vtf` | `b0f6c3daa541b8aba8adf3164e4f6df8693ab6d40820732e8dfa7f415b8ec9a9` | `41dc0f9966d3e3a82539e7ec6586f90ce20b840bf64dbb32c42cb34a8fb647e7` | 19,309 |
| `materials/signs/number_09.vtf` | `bc2bb926f4a9b8248b0585f4fcbe756b8871e30fabb348124b7a50da7a0a7988` | `b913cf57a5717b130e12f930dfc1d24f4cdc96468d17deedf5f92cdac84960d1` | 19,400 |
| `materials/signs/arrow_lt_blue.vtf` | `265f086c053767670f8a2198abac0f36c9240b74db857e4a5e1e8040c1f35a98` | `f6b9eb69df5c89b77efdd210c1f8e2c07f433026aad6fd4846b61d374f167d0d` | 19,360 |
| `materials/signs/arrow_rt_blue.vtf` | `fbd5ddeeeeeae3b48eef125de43f44ed4997cd01eba64cc7cbc52a514853d6b3` | `79b6ae9002c5e873ed98365d83a569df5715cffb541e8dce2715cdc7a27993f1` | 19,360 |
| `materials/signs/arrow_up_blue.vtf` | `b57929423f610bf6329381cbaca989c6f7d37b110f29d854009d4c2786f14684` | `b5975e6c27f6b204675caabdbe0128c3a29306a2b6e965adda6732aa04570100` | 19,360 |

Every present decal plane contains alpha-zero texels. An opaque rectangular background is therefore not a VTF decode disposition and must not replace these alpha bytes downstream.

## Sky textures

LDR sky rows use BGR888 code 3 and flags `0x0000034c`; HDR compressed rows use BGRA8888 code 12 and flags `0x0000230d`. Every row is VTF 7.3 with one frame. The four horizontal faces are 512×256 with ten mips, up is 512×512 with ten mips, and down is 4×4 with three mips.

| Logical path | SHA-256 |
|---|---|
| `materials/skybox/sky_day01_01rt.vtf` | `05ede6ab9ca100e635e6840127af64323294081102c2a57b7b1931324dde7d6f` |
| `materials/skybox/sky_day01_01lf.vtf` | `7ed2533e273546d8cf49d44da6a40955c45e7c012f2893cf7bd6d34da58e7a52` |
| `materials/skybox/sky_day01_01bk.vtf` | `2d757e215a45151378c4fc00f45b5069cf651754ed2697ea5ef15a7f80dca5dc` |
| `materials/skybox/sky_day01_01ft.vtf` | `5ccc101c936cfb36b4046aa87172db742bb4f1632a821ab3f43b0bfe201c26f2` |
| `materials/skybox/sky_day01_01up.vtf` | `eebb7bc329928dc0a7afa1044d32a4697c7bb7c3f645132fb47eb0fb749a6895` |
| `materials/skybox/sky_day01_01dn.vtf` | `3b339fa596941492f630af8663d1cbf5fd6a28e87388e5ace87570287fae4cdc` |
| `materials/skybox/sky_day01_01_hdrrt.vtf` | `1bc24c7d7cbe91626511e9bcd3c7b4da8b9126a7b2fa366923d20429fa64e043` |
| `materials/skybox/sky_day01_01_hdrlf.vtf` | `0cdf8278ce8b0b7f78b77cee6917dfaf4c2aeb4c0f6127dbd2e5cc963173e948` |
| `materials/skybox/sky_day01_01_hdrbk.vtf` | `cf8605896a06634ad224d2eb1f19345e73c28d70797665553c5e9a81cf8ad96f` |
| `materials/skybox/sky_day01_01_hdrft.vtf` | `27c20932c106935d5b59cf4c3fec9a831b9a4f4c053bfd082f0b18862af50828` |
| `materials/skybox/sky_day01_01_hdrup.vtf` | `2281d9adb14caa4d12e06e4df9fa3c5862f42ebaebfee5ddb0d2e1a2e5ec3b23` |
| `materials/skybox/sky_day01_01_hdrdn.vtf` | `6a9507c9cee4162994678f07559d769411111b3b5960b57aa6b743829e22f069` |

## Cubemap textures

All six are VTF 7.4, 32×32, six mips, one frame, and seven Source-2013 PC faces. LDR rows use BGR888 code 3 and flags `0x0000434c`; HDR rows use RGBA16F code 24 and flags `0x0000600c`.

| Logical path | SHA-256 |
|---|---|
| `materials/maps/jump_beef/c-4787_3137_-2159.vtf` | `627caf57bfe16e869a64b282d6bc39663ff4682cde2c5b244772a617db2a353a` |
| `materials/maps/jump_beef/c12672_539_-2562.vtf` | `627caf57bfe16e869a64b282d6bc39663ff4682cde2c5b244772a617db2a353a` |
| `materials/maps/jump_beef/c12672_683_-4448.vtf` | `627caf57bfe16e869a64b282d6bc39663ff4682cde2c5b244772a617db2a353a` |
| `materials/maps/jump_beef/c-4787_3137_-2159.hdr.vtf` | `b3f13af032931bd21fc6f9c873f07b3718b1dc2a8c86ccebdca514e8826ab7e8` |
| `materials/maps/jump_beef/c12672_539_-2562.hdr.vtf` | `c389984d1f4f1941b36da93e18c012d965fe91e96e16d03a1dda637cea814f19` |
| `materials/maps/jump_beef/c12672_683_-4448.hdr.vtf` | `a5e761098727e2babe50e59fa27ef293d0b49d4293f824a64474c4ccd07026c9` |
