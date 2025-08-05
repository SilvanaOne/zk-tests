use crypto_bigint::{U256, NonZero};

#[cfg(target_os = "zkvm")]
extern "C" {
    /// SP1 syscall for big integer operations with a modulus.
    fn sys_bigint(
        result: *mut [u32; 8],
        op: u32,
        x: *const [u32; 8],
        y: *const [u32; 8],
        modulus: *const [u32; 8],
    );
}

#[cfg(target_os = "zkvm")]
const OP_MULTIPLY: u32 = 0;

#[derive(Clone)]
pub struct PoseidonConfig {
    pub mds: Vec<Vec<U256>>,
    pub round_constants: Vec<Vec<U256>>,
    pub full_rounds: usize,
    pub has_initial_round_constant: bool,
    pub state_size: usize,
    pub rate: usize,
    pub power: u32,
}

pub struct PoseidonConstant;

impl PoseidonConstant {
    pub fn poseidon_config_kimchi_fp() -> PoseidonConfig {
        PoseidonConfig {
            mds: vec![
                vec![
                    U256::from_be_hex("1a9bd250757e29ef4959b9bef59b4e60e20a56307d6491e7b7ea1fac679c7903"),
                    U256::from_be_hex("384aa09faf3a48737e2d64f6a030aa242e6d5d455ae4a13696b48a7320c506cd"),
                    U256::from_be_hex("3d2b7b0209bc3080064d5ce4a7a03653f8346506bfa6d076061217be9e6cfed5"),
                ],
                vec![
                    U256::from_be_hex("09ee57c70bc351220b107983afcfabbea79868a4a8a5913e24b7aaf3b4bf3a42"),
                    U256::from_be_hex("20989996bc29a96d17684d3ad4c859813115267f35225d7e1e9a5b5436a2458f"),
                    U256::from_be_hex("14e39adb2e171ae232116419ee7f26d9191edde8a5632298347cdb74c3b2e69d"),
                ],
                vec![
                    U256::from_be_hex("174544357b687f65a9590c1df621818b5452d5d441597a94357f112316ef67cb"),
                    U256::from_be_hex("3ca9263dc1a19d17cfbf15b0166bb25f95dffc53212db207fcee35f02c2c4137"),
                    U256::from_be_hex("3cf1fbef75d4ab63b7a812f80b7b0373b2dc21d269ba7c4c4d6581d50aae114c"),
                ],
            ],
            round_constants: vec![
                vec![
                    U256::from_be_hex("2ec559cd1a1f2f6889fc8ae5f07757f202b364429677c8ff6603fd6d93659b47"),
                    U256::from_be_hex("2553b08c788551bfe064d91c17eb1edb8662283229757711b2b30895f0aa3bad"),
                    U256::from_be_hex("25a706fb0f35b260b6f28d61e082d36a8f161be1f4d9416371a7b65f2bfafe4e"),
                ],
                vec![
                    U256::from_be_hex("37c0281fda664cc2448d0e7dd77aaa04752250817a945abeea8cfaaf3ee39ba0"),
                    U256::from_be_hex("140488321291998b8582eaceeb3fa9ca3980eb64a453573c5aaa2910405936b6"),
                    U256::from_be_hex("3a73fe35b1bdd66b809aad5eab47b5c83b0146fd7fc632dfb49cd91ae1169378"),
                ],
                vec![
                    U256::from_be_hex("21b7c2b35fd7710b06245711f26c0635d3e21de4db10dd3a7369f59f468d7be6"),
                    U256::from_be_hex("1803a068d25fef2ef652c8a4847aa18a29d1885e7bf77fd6a34d66536d09cad7"),
                    U256::from_be_hex("291de61c5e6268213772cf7e03c80c2e833eb77c58c46548d158a70fbbd9724b"),
                ],
                vec![
                    U256::from_be_hex("230043a0dc2dfab63607cbe1b9c482fdd937fdefecc6905aa5012e89babead13"),
                    U256::from_be_hex("218af77a05c502d3fa3144efcf47a0f2a0292498c10c6e2368565674e78764f4"),
                    U256::from_be_hex("223e2d94c177d27e071d55729d13a9b216955c7102cc9a95ea40058efb506117"),
                ],
                vec![
                    U256::from_be_hex("2a18257c15ad9b6fe8b7c5ad2129394e902c3c3802e738f24ce2f585ae5f6a38"),
                    U256::from_be_hex("0a6f7ba75f216403d2e4940469d199474a65aa5ef814e36400bddef06158dcf8"),
                    U256::from_be_hex("169be41c6227956efef5b4cdde65d00d5e04fe766178bdc731615c6e5b93e31e"),
                ],
                vec![
                    U256::from_be_hex("2e28f50a9a55d2e91774083072734544417e290a1cfebc01801b94d0728fe663"),
                    U256::from_be_hex("0fdedf8da8654a22831040cfc74432464b173ee68628fd90498480b9902f2819"),
                    U256::from_be_hex("046a3ed9863d2d739dd8bc9e90a746fda1197162d0a0bec3db1f2f6042cf04e2"),
                ],
                vec![
                    U256::from_be_hex("219e08b460c305b428670bacab86ac1e9458075778d35c3619ae7ba1f9b2ed76"),
                    U256::from_be_hex("38bb36a12ebcec4d4e8728eb43e3f12a6e33b1ffa1463379018d4e12424e62ca"),
                    U256::from_be_hex("1e9aa3fe25d116ccfbd6a8fccdae0aa9bc164a03ab7e951704ee9a715fbedee6"),
                ],
                vec![
                    U256::from_be_hex("030f33ed70da4c2bfb844ff1a7558b817d1ec300da86a1694f2db45047d5f18b"),
                    U256::from_be_hex("0282b04137350495ab417cf2c47389bf681c39f6c22d9e370b7af75cbcbe4bb1"),
                    U256::from_be_hex("09b1528dea2eb5bd96905b88ff05fdf3e0f220fe1d93d1b54953ac98fec825f0"),
                ],
                vec![
                    U256::from_be_hex("30083dbbb5eab39311c7a8bfd5e55567fa864b3468b5f9200e529cda03d9ef71"),
                    U256::from_be_hex("017eace73cf67c6112239cbf51dec0e714ee4e5a91dbc9209dc17bbea5bcd094"),
                    U256::from_be_hex("37af1de8f5475ba165b90f8d568683d54e215df97e9287943370cf4118428097"),
                ],
                vec![
                    U256::from_be_hex("16ff7592836a45340ec6f2b0f122736d03f0bcb84012f922a4baa73ea0e66f51"),
                    U256::from_be_hex("1a5985d4b359d03de60b2edabb1853f476915febc0e40f83a2d1d0084efc3fd9"),
                    U256::from_be_hex("255a9d4beb9b5ea18ab9782b1abb267fc5b773b98ab655fd4d469698e1e1f975"),
                ],
                vec![
                    U256::from_be_hex("34a8d9f45200a9ac28021712be81e905967bac580a0b9ee57bc4231f5ecb936a"),
                    U256::from_be_hex("0979556cb3edcbe4f33edd2094f1443b4b4ec6c457b0425b8463e788b9a2dcda"),
                    U256::from_be_hex("2a4d028c09ad39c30666b78b45cfadd5279f6239379c689a727f626679272654"),
                ],
                vec![
                    U256::from_be_hex("0c31b68f6850b3bd71fe4e89984e2c87415523fb54f24ec8ae71430370154b33"),
                    U256::from_be_hex("1a27ca0b953d3dba6b8e01cf07d76c611a211d139f2dff5ac023ed2454f2ed90"),
                    U256::from_be_hex("109ae97c25d60242b86d7169196d2212f268b952dfd95a3937916b9905303180"),
                ],
                vec![
                    U256::from_be_hex("3698c932f2a16f7bb9abac089ec2de79c9965881708878683caf53caa83ad9c4"),
                    U256::from_be_hex("3c7e25e0ac8fba3dc1360f8a9a9fa0be0e031c8c76a93497b7cac7ed32ade6c0"),
                    U256::from_be_hex("2fc5023c5e4aed5aa7dfca0f5492f1b6efab3099360ec960237512f48c858a79"),
                ],
                vec![
                    U256::from_be_hex("2c124735f3f924546fb4fdfa2a018e03f53063d3a2e87fd285ba8d647eda6765"),
                    U256::from_be_hex("12c875c9b79591acf9033f8b6c1e357126c44b23f3486fbee0d98340a3382251"),
                    U256::from_be_hex("3cda935e895857d39a7db8476aeda5a5131cb165a353073fd3e473fd8855528d"),
                ],
                vec![
                    U256::from_be_hex("218eb756fa5f1df9f1eb922ef80b0852588779a7368e3d010def1512815d8759"),
                    U256::from_be_hex("23bcf1032957015ef171fbb4329bca0c57d59885522f25f4b082a3cf301cfbc6"),
                    U256::from_be_hex("17474c3b6a9bc1057df64b9e4d62badbc7f3867b3dd757c71c1f656205d7bceb"),
                ],
                vec![
                    U256::from_be_hex("019826c0ee22972deb41745d3bd412c2ae3d4c18535f4b60c9e870edffa3d550"),
                    U256::from_be_hex("30bcb17dfd622c46f3275f698319b68d8816bed0368ded435ed61992bc43efa9"),
                    U256::from_be_hex("3bd816c214c66410229cfbd1f4a3a42e6a0f82f3c0d49b09bc7b4c042ff2c94b"),
                ],
                vec![
                    U256::from_be_hex("08943ec01d9fb9f43c840757738979b146c3b6d1982280e92a52e8d045633ea1"),
                    U256::from_be_hex("2670bf8c01822e31c70976269d89ed58bc79ad2f9d1e3145df890bf898b57e47"),
                    U256::from_be_hex("0dd53b41599ae78dbd3e689b65ebcca493effa94ed765eeec75a0d3bb20407f9"),
                ],
                vec![
                    U256::from_be_hex("068177d293585e0b8c8e76a8a565c8689a1d88e6a9afa79220bb0a2253f203c3"),
                    U256::from_be_hex("35216f471043866edc324ad8d8cf0cc792fe7a10bf874b1eeac67b451d6b2cf5"),
                    U256::from_be_hex("1fd6efb2536bfe11ec3736e7f7448c01eb2a5a9041bbf84631cc83ee0464f6af"),
                ],
                vec![
                    U256::from_be_hex("2c982c7352102289fc1b48dafcd9e3cc364d5a4324575e4721daf0af10033c67"),
                    U256::from_be_hex("352f7e8c7662d86db9c722d4d07778858771b832af5bb5dc3b13cf94851c1b45"),
                    U256::from_be_hex("18e3c0c1caa5e3ed66ee1ab6f55a5c8063d8c9b034ae47db43435147149e37d5"),
                ],
                vec![
                    U256::from_be_hex("3124b12deb37dcbb3d96c1a08d507523e30e03e0919559bf2daaab238422eade"),
                    U256::from_be_hex("143bf0def31437eb21095200d2d406e6e5727833683d9740b9bfc1713215dc9a"),
                    U256::from_be_hex("1ebee92143f32b4f9d9a90ad62b8483c977480767b53c71f6bde934a8ef38f17"),
                ],
                vec![
                    U256::from_be_hex("0ff6c794ad1afaa494088d5f8ee6c47bf9e83013478628cf9f41f2e81383ebeb"),
                    U256::from_be_hex("3d0a10ac3ee707c62e8bdf2cdb49ac2cf4096cf41a7f214fdd1f8f9a24804f17"),
                    U256::from_be_hex("1d61014cd3ef0d87d037c56bdfa370a73352b95d472ead1937bed06a31801c91"),
                ],
                vec![
                    U256::from_be_hex("123e185b2ec7f072507ac1e4e743589bb25c8fdb468e329e7de169875f90c525"),
                    U256::from_be_hex("30b780c0c1cb0609623732824c75017da9799bdc7e08b527bae7f409ebdbecf2"),
                    U256::from_be_hex("1dfb3801b7ae4e209f68195612965c6e37a2ed5cf1eeee3d46edf655d6f5afef"),
                ],
                vec![
                    U256::from_be_hex("2fdee42805b2774064e963c741552556019a9611928dda728b78311e1f049528"),
                    U256::from_be_hex("31b2b65c431212ed36fdda5358d90cd9cb51c9f493bff71cdc75654547e4a22b"),
                    U256::from_be_hex("1e3ca033d8413b688db7a543e62ac2e69644c0614801379cfe62fa220319e0ef"),
                ],
                vec![
                    U256::from_be_hex("0c8ef1168425028c52a32d93f9313153e52e9cf15e5ec2b4ca09d01730dad432"),
                    U256::from_be_hex("378c73373a36a5ed94a34f75e5de7a7a6187ea301380ecfb6f1a22cf8552638e"),
                    U256::from_be_hex("3218aeec20048a564015e8f221657fbe489ba404d7f5f15b829c7a75a85c2f44"),
                ],
                vec![
                    U256::from_be_hex("3312ef7cbbad31430f20f30931b070379c77119c1825c6560cd2c82cf767794e"),
                    U256::from_be_hex("356449a71383674c607fa31ded8c0c0d2d20fb45c36698d258cecd982dba478c"),
                    U256::from_be_hex("0cc88d1c91481d5321174e55b49b2485682c87fac2adb332167a20bcb57db359"),
                ],
                vec![
                    U256::from_be_hex("1defccbd33740803ad284bc48ab959f349b94e18d773c6c0c58a4b9390cc300f"),
                    U256::from_be_hex("2d263cc2e9af126d768d9e1d2bf2cbf32063be831cb1548ffd716bc3ee7034fe"),
                    U256::from_be_hex("111e314db6fb1a28e241028ce3d347c52558a33b6b11285a97fffa1b479e969d"),
                ],
                vec![
                    U256::from_be_hex("027409401e92001d434cba2868e9e371703199c2372d23ef329e537b513f453e"),
                    U256::from_be_hex("24a852bdf9cb2a8fedd5e85a59867d4916b8a57bdd5f84e1047d410770ffffa0"),
                    U256::from_be_hex("205d1b0ee359f621845ac64ff7e383a3eb81e03d2a2966557746d21b47329d6e"),
                ],
                vec![
                    U256::from_be_hex("25c327e2cc93ec6f0f23b5e41c931bfbbe4c12da7d55a2b1c91c79db982df903"),
                    U256::from_be_hex("39df3e22d22b09b4265da50ef175909ce79e8f0b9599dff01cf80e70884982b9"),
                    U256::from_be_hex("09b08d58853d8ac908c5b14e5eb8611b45f40faaa59cb8dff98fb30efcdfaa01"),
                ],
                vec![
                    U256::from_be_hex("1ece62374d79e717db4a68f9cddaaf52f8884f397375c0f3c5c1dbaa9c57a0a6"),
                    U256::from_be_hex("3bd089b727a0ee08e263fa5e35b618db87d7bcce03441475e3fd49639b9fa1c1"),
                    U256::from_be_hex("3fedea75f37ad9cfc94c95141bfb4719ee9b32b874b93dcfc0cc12f51a7b2aff"),
                ],
                vec![
                    U256::from_be_hex("36dfa18a9ba1b194228494a8acaf0668cb43aca9d4e0a251b20ec3424d0e65cd"),
                    U256::from_be_hex("119e98db3f49cd7fcb3b0632567d9ccaa5498b0d411a1437f57c658f41931d0c"),
                    U256::from_be_hex("1100b21c306475d816b3efcd75c3ae135c54ad3cc56ca22abd9b7f45e6d02c19"),
                ],
                vec![
                    U256::from_be_hex("15791f9bbea213937208c82794eb667f157f003c65b64aa9800f4bbee4ea5119"),
                    U256::from_be_hex("1adbeb5e9c4d515ecfd250ebee56a2a816eb3e3dc8d5d440c1ab4285b350be64"),
                    U256::from_be_hex("1fbf4738844a9a249aec253e8e4260e4ab09e26bea29ab0020bf0e813ceecbc3"),
                ],
                vec![
                    U256::from_be_hex("3418a929556ec51a086459bb9e63a821d407388cce83949b9af3e3b0434eaf0e"),
                    U256::from_be_hex("09406b5c3af0290f997405d0c51be69544afb240d48eeab1736cda0432e8ff9e"),
                    U256::from_be_hex("23ece5d70b38ccc9d43cd923e5e3e2f62d1d873c9141ef01f89b6de1336f5bc7"),
                ],
                vec![
                    U256::from_be_hex("1852d574e46d370a0b1e64f6c41eeb8d40cf96c524a62965661f2ef87e67234d"),
                    U256::from_be_hex("0a657027cce8d4f238ea896dde273b7537b508674a366c66b3789d9828b0ce90"),
                    U256::from_be_hex("3482f98a46ec358108fbbb68fd94f8f2baa73c723baf21922a850e45511f5a2d"),
                ],
                vec![
                    U256::from_be_hex("3f62f164f8c905b335a6cbf76131d2430237e17ad6abc76d2a6329c1ec5463ee"),
                    U256::from_be_hex("07e397f503f9c1cea028465b2950ea444b15c5eab567d5a69ea2925685694df0"),
                    U256::from_be_hex("0405f1fc711872373d6eb50a09fbfb05b2703ae0a0b4edb86aedb216db17a876"),
                ],
                vec![
                    U256::from_be_hex("0be0848eb3e09c7027110ad842c502441c97afa14a844406fcfec754a25658c1"),
                    U256::from_be_hex("26b78788fd98ac020bac92d0e7792bb5ffed06b697d847f61d984f905d9ba870"),
                    U256::from_be_hex("38fd5318d39055c82fef9bdd33315a541c0ec4363e6cc0687005871355dfa573"),
                ],
                vec![
                    U256::from_be_hex("380bd03b840c48c8ba3830e7cace72f91a5002218c617294e8c8bc687d5216de"),
                    U256::from_be_hex("2c6e57ddc1d7c81a0299ed49c3d74759416bc8426f30e2af5622895c531b4e1c"),
                    U256::from_be_hex("11d3a81b262fc76ef506ee6d88e5991d0de8cb9dd162d97c58b175e3bc4584f3"),
                ],
                vec![
                    U256::from_be_hex("09b6b283ebaf45fbb1e448969ace9be62adf67ddf58614925741deb6a1ba7def"),
                    U256::from_be_hex("15d5095164c885763fa83cdf776d436382821a17bc5563a5b6f6dfcdac504ade"),
                    U256::from_be_hex("3427fdbfca3cea23063eb138c5055c6cad9c4252b23d12c12293308eff7d9124"),
                ],
                vec![
                    U256::from_be_hex("272f12e731077b74317ef2543c33b86194db1da5f6a7e1eee0656672c81685fe"),
                    U256::from_be_hex("05323f85deb8c07c193c37a73d76f6114967913a2bdce11995f183e769f42967"),
                    U256::from_be_hex("3d5ce415ecae4ba42b417ea3a501b44694f46efddff2fcca952b097f3852d3d8"),
                ],
                vec![
                    U256::from_be_hex("0e8ec18c7b52c514d42047f1f0b2a90cb8c0c7391cf9479cd7fd5bfe1d3db8f2"),
                    U256::from_be_hex("01591c865ea7065d54304519f8bb268bddbeaf3afae54edcd01a833ed0a9ef1a"),
                    U256::from_be_hex("3eddbeeee5eca5deee4bf1789c435e1241e0d71186d8f0f62d74729dfc3119fb"),
                ],
                vec![
                    U256::from_be_hex("23691c7009b9283b268766e8d491716d3c1993e6ecf458def8f762af3e355707"),
                    U256::from_be_hex("26cdab2c837ebeac5bea4be1d6f0488034907374d81a61a34f1c4db397d4c09b"),
                    U256::from_be_hex("2d2206730664d58be0676dad1fee0e990c264a7410a2cdb6b55653c1df72ef56"),
                ],
                vec![
                    U256::from_be_hex("2bb74bb185372334a4ef5f6d18e2ece54086e62b04985dd794b7117b0be9217f"),
                    U256::from_be_hex("366250fe928c45d8d5aa35f0a142754907ff3c598410199b589b28cd851b2204"),
                    U256::from_be_hex("1868f8118482c6b4a5a61a81c8aaca128953179c20f73a44022d9976bdc34af1"),
                ],
                vec![
                    U256::from_be_hex("0b7901c670e1d75d726eb88d000950b3c963f0f7a6ca24994bdc07ae2f78b4d3"),
                    U256::from_be_hex("032c4bd8ab70e1f25af77af57dd340c8e6c8a101dfc5e8dd03314566db90b870"),
                    U256::from_be_hex("1ce36db31fe6ea3cd9308db9aa43a8af5c41a8f0a6509bfe00f0e7b486c0ab8a"),
                ],
                vec![
                    U256::from_be_hex("26596ea9e1915e53da3479e9d13c3c920505e2449e325810ff6ca855fe4b7c6e"),
                    U256::from_be_hex("30f296a269868a7fca8f5b1e269c0116304df31729559a270e713509d3a6d5dc"),
                    U256::from_be_hex("02588961eff7897d87eb6ac72350ef9f52640647cbd23136919a994dfd1979d5"),
                ],
                vec![
                    U256::from_be_hex("16a49e69721e80690d41e06229e9bc2dbaf9a2abf4b89388db2485595409d62b"),
                    U256::from_be_hex("3d7aca02c051fcad8073cfd67210cd423a31888afc4a444d9d3adf3d6c5da7bf"),
                    U256::from_be_hex("299bd48a740b7790075268312ab8072c72421de5a6437fa5e25431ef951847b4"),
                ],
                vec![
                    U256::from_be_hex("11a69b867d9ea22ec1b2f28e96617129e36eefaea9e8126bdc6a42b99072902b"),
                    U256::from_be_hex("25bc1af391f3c1f2284a95da92b5883d1b3a40794b2358b2e7a70fca22da64ce"),
                    U256::from_be_hex("361ab3843f4d8ddadede39d82bb1a8109f89b6d9aa117b8f365de43895de0baa"),
                ],
                vec![
                    U256::from_be_hex("38ef3ab5b61c117a3465a017a9c8ba4c227659b41fdf145206d5c960f49dd45b"),
                    U256::from_be_hex("3992f83f26143dbdbd335604a1a14daf238ae43c249783f694feaf560aaae20f"),
                    U256::from_be_hex("350287977eb71c81b10ecd039aad99cfa9ed84a04301cb30869e1dc7fa1dc638"),
                ],
                vec![
                    U256::from_be_hex("3afb5bc126020586dcccba32dd054cd9a3f3b834ca9678d6802c48b1da97d6ed"),
                    U256::from_be_hex("172b7c2d8e7e4b06d183a2575b790749d0970c54966407fa8f59072c729de671"),
                    U256::from_be_hex("2eb53fe3a278688a70494569e54a0f0d269935aec6c897bef4d368c1f67d57e4"),
                ],
                vec![
                    U256::from_be_hex("0375ae56b8d9310d553ed77d406dedc3f0393e5a321b71caee6a5bb7078b5035"),
                    U256::from_be_hex("1d49a0d53bc2993cbf1fb5d1da9bb76fe46a7031d5e5d43fadbf54bc17c1ef38"),
                    U256::from_be_hex("132d17b87cab6d707ddfa1f01df1724ad37957e989c44f1ff71426367f953160"),
                ],
                vec![
                    U256::from_be_hex("062da5280948d8c6c4acc7e6a1aa421f0f9ec179a44146750060be4be6755f85"),
                    U256::from_be_hex("0a4b4d5cde54a974ea4e57ee4132d2ab2510c300f21930d6bbbf211d1add80f9"),
                    U256::from_be_hex("3356f1fbeac493ccab752b70bbed821ce49965c19284d7aacd78fbf3ff864e91"),
                ],
                vec![
                    U256::from_be_hex("042721e8a9cc32557851feb0e0190c5dfbf4cb1b8f47d37e7e653ec6ff8a4059"),
                    U256::from_be_hex("053d9b2633fff31ca4fc5724ce6b4422318128cdf01897d321e86f47cdf748b1"),
                    U256::from_be_hex("267d96caeafde5dbd3db1f0668b09ccd532a22f0205494716a786219fb4c801c"),
                ],
                vec![
                    U256::from_be_hex("39316997737610193c3f9ffcfd4e23d38aac12cd7b95b8d256d774101650a6ca"),
                    U256::from_be_hex("191e377462986563fdabf9b23529f7c84c6b200b9101b3a5096bca5f377981fb"),
                    U256::from_be_hex("20f89af9722f79c860d2059a0ec209cf3a7925ad0798cab655eca62fe73ff3d9"),
                ],
                vec![
                    U256::from_be_hex("1ca568aeddb2ef391a7c78ecf104d32d785b9ca145d97e35879df3534a7d1e0b"),
                    U256::from_be_hex("25de9ba0a37472c3b4c0b9c3bc25cbbf78d91881b6f94ee70e4abf090211251c"),
                    U256::from_be_hex("3393debd38d311881c7583bee07e605ef0e55c62f0508ccc2d26518cd568e1ef"),
                ],
                vec![
                    U256::from_be_hex("038df2fd18a8d7563806aa9d994a611f642d5c397388d1dd3e78bc7a4515c5b1"),
                    U256::from_be_hex("05c6503ff1ee548f2435ad9148d7fb94c9222b0908f445537a6667047f6d501c"),
                    U256::from_be_hex("104c88d6d0682d82d3d664826dc9565db101a220aa8f90572eb798468a82a2ab"),
                ],
                vec![
                    U256::from_be_hex("2caad6108c09ee6aee7851b4a2d2d3b7c3ca3c56a80003c8471f90bfa4ac628b"),
                    U256::from_be_hex("0a57dbd4c327826c8a97bc7285f94bcddb966177346f1792c4bd7088aa0353f3"),
                    U256::from_be_hex("3c15552f9124318b8433d01bb53ba04ba1cc9eb91d83b918e32fea39fbe908fa"),
                ],
                vec![
                    U256::from_be_hex("0e10c10cbbe1717a9441c6299c4fc087c222208bd4fa8f3be66d2075f623b513"),
                    U256::from_be_hex("1e8b254cbff2c92a83dff1728c81dd22a9570f590e497cb2d640042cb879a930"),
                    U256::from_be_hex("1812dbcd70c440610057bbfdd0cc4d31d1faf5786419b53841c4adc43f2b2352"),
                ],
            ],
            full_rounds: 55,
            has_initial_round_constant: false,
            state_size: 3,
            rate: 2,
            power: 7,
        }
    }
}

pub struct FiniteField;

impl FiniteField {
    pub fn mod_p(x: &U256, p: &U256) -> U256 {
        // For the Mina prime, we can optimize by checking if reduction is needed
        if x < p {
            *x
        } else {
            let p_nonzero = NonZero::new(*p).expect("Modulus cannot be zero");
            x.rem(&p_nonzero)
        }
    }

    pub fn power(a: &U256, n: &U256, p: &U256) -> U256 {
        let mut a = Self::mod_p(a, p);
        let mut x = U256::ONE;
        let mut n = *n;

        // For power 7 (used in Poseidon), we can optimize:
        // a^7 = a * a^2 * a^4 = a * a^2 * (a^2)^2
        if n == U256::from(7u32) {
            let a2 = Self::mul(&a, &a, p);    // a^2
            let a4 = Self::mul(&a2, &a2, p);  // a^4
            let a3 = Self::mul(&a2, &a, p);   // a^3 = a^2 * a
            return Self::mul(&a4, &a3, p);    // a^7 = a^4 * a^3
        }

        // General case for other exponents
        while n > U256::ZERO {
            if n.bit(0).into() {
                x = Self::mul(&x, &a, p);
            }
            a = Self::mul(&a, &a, p);
            n = n.shr(1);
        }
        x
    }

    pub fn dot(x: &[U256], y: &[U256], p: &U256) -> U256 {
        let mut z = U256::ZERO;
        let n = x.len();
        
        // Compute all products first, then sum them
        // This can help with instruction scheduling
        for i in 0..n {
            let prod = Self::mul(&x[i], &y[i], p);
            z = z.add_mod(&prod, p);
        }
        z
    }

    pub fn add(x: &U256, y: &U256, p: &U256) -> U256 {
        // Use the optimized add_mod from crypto-bigint
        // This is more efficient than our manual implementation
        x.add_mod(y, p)
    }

    pub fn mul(x: &U256, y: &U256, p: &U256) -> U256 {
        let x_mod = Self::mod_p(x, p);
        let y_mod = Self::mod_p(y, p);
        
        #[cfg(target_os = "zkvm")]
        {
            // Use direct syscall for modular multiplication in zkVM
            // This provides maximum efficiency by bypassing all software implementation
            let result = unsafe {
                let mut out = [0u32; 8];
                sys_bigint(
                    &mut out as *mut [u32; 8],
                    OP_MULTIPLY,
                    x_mod.to_words().as_ptr() as *const [u32; 8],
                    y_mod.to_words().as_ptr() as *const [u32; 8],
                    p.to_words().as_ptr() as *const [u32; 8],
                );
                U256::from_words(out)
            };
            
            // The syscall should return a value less than the modulus
            debug_assert!(result < *p);
            return result;
        }
        
        #[cfg(not(target_os = "zkvm"))]
        {
            // Fallback implementation for non-zkVM environments
            // First check if we can do simple multiplication without overflow
            let p_nonzero = NonZero::new(*p).expect("Modulus cannot be zero");
            
            // Try to use the most efficient path based on the size of operands
            // If both operands are small enough, we can use direct multiplication
            let x_high = x_mod.shr(128);
            let y_high = y_mod.shr(128);
            
            if x_high == U256::ZERO && y_high == U256::ZERO {
                // Both values fit in 128 bits, so product fits in 256 bits
                let product = x_mod.wrapping_mul(&y_mod);
                product.rem(&p_nonzero)
            } else {
                // Use Montgomery multiplication algorithm for efficiency
                // This is still optimized by the SP1 patches at the lower level
                let mut result = U256::ZERO;
                let mut a = x_mod;
                let b = y_mod;
                
                // Standard double-and-add multiplication with modular reduction
                for i in 0..256 {
                    if b.bit(i).into() {
                        result = Self::add(&result, &a, p);
                    }
                    a = Self::add(&a, &a, p);
                }
                
                result
            }
        }
    }
}

pub struct PoseidonHash;

impl PoseidonHash {
    // Prime field modulus for Mina
    pub fn p() -> U256 {
        U256::from_be_hex("40000000000000000000000000000000224698fc094cf91b992d30ed00000001")
    }

    /// Main hash function - equivalent to the C# Hash method
    pub fn hash(input: Vec<U256>) -> U256 {
        let initial_state = vec![U256::ZERO, U256::ZERO, U256::ZERO];
        let config = PoseidonConstant::poseidon_config_kimchi_fp();
        Self::poseidon_update(initial_state, input, &config)[0]
    }

    pub fn poseidon_update(
        mut state: Vec<U256>,
        input: Vec<U256>,
        config: &PoseidonConfig,
    ) -> Vec<U256> {
        if input.is_empty() {
            Self::permutation(&mut state, config);
            return state;
        }

        // Pad input with zeros so its length is a multiple of the rate
        let n = ((input.len() as f64 / config.rate as f64).ceil() as usize) * config.rate;
        let mut array = vec![U256::ZERO; n];

        // Copy input to array
        for (i, val) in input.iter().enumerate() {
            array[i] = *val;
        }

        let p = Self::p();

        // For every block of length `rate`, add block to the first `rate` elements of the state, and apply the permutation
        for block_index in (0..n).step_by(config.rate) {
            for i in 0..config.rate {
                state[i] = state[i].add_mod(&array[block_index + i], &p);
            }
            Self::permutation(&mut state, config);
        }

        state
    }

    pub fn permutation(state: &mut Vec<U256>, config: &PoseidonConfig) {
        let p = Self::p();

        // Special case: initial round constant
        let mut offset = 0;
        if config.has_initial_round_constant {
            for i in 0..config.state_size {
                state[i] = state[i].add_mod(&config.round_constants[0][i], &p);
            }
            offset = 1;
        }

        // Precompute power for all rounds (it's always 7)
        let power_n = U256::from(config.power as u64);
        
        for round in 0..config.full_rounds {
            // Raise to a power - optimize for power 7
            if config.power == 7 {
                for i in 0..config.state_size {
                    let x = &state[i];
                    let x2 = FiniteField::mul(x, x, &p);      // x^2
                    let x4 = FiniteField::mul(&x2, &x2, &p);  // x^4
                    let x3 = FiniteField::mul(&x2, x, &p);    // x^3
                    state[i] = FiniteField::mul(&x4, &x3, &p); // x^7
                }
            } else {
                for i in 0..config.state_size {
                    state[i] = FiniteField::power(&state[i], &power_n, &p);
                }
            }

            // Matrix multiplication with round constant addition
            let mut new_state = vec![U256::ZERO; config.state_size];
            for i in 0..config.state_size {
                // Compute dot product and add round constant in one pass
                let mut acc = config.round_constants[round + offset][i];
                for j in 0..config.state_size {
                    let prod = FiniteField::mul(&config.mds[i][j], &state[j], &p);
                    acc = acc.add_mod(&prod, &p);
                }
                new_state[i] = acc;
            }
            *state = new_state;
        }
    }
}

pub fn poseidon(input: Vec<U256>) -> U256 {
    PoseidonHash::hash(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypto_bigint::{U256, Encoding};

    #[test]
    fn test_poseidon_hash_123() {
        let input = vec![U256::ONE, U256::from(2u64), U256::from(3u64)];
        let result = poseidon(input);
        println!("Poseidon hash for [1, 2, 3]: {:?}", result);
        // Add assertion once expected hash is known
        assert_eq!(
            result,
            U256::from_be_hex("366e46102b0976735ed1cc8820c7305822a448893fee8ceeb42a3012a4663fd0"),
        );
    }

    #[test]
    fn test_crypto_bigint_arithmetic() {
        // Create U256 values for 2 and 3
        let two = U256::from(2u64);
        let three = U256::from(3u64);

        // Calculate 2 + 3
        let sum = two.wrapping_add(&three);
        println!("2 + 3 = {:?}", sum);

        // Assert that 2 + 3 = 5
        assert_eq!(sum, U256::from(5u64));
        assert_eq!(sum.to_be_bytes()[31], 5u8); // Check the least significant byte

        // Calculate 2 * 3
        let product = two.wrapping_mul(&three);
        println!("2 * 3 = {:?}", product);

        // Assert that 2 * 3 = 6
        assert_eq!(product, U256::from(6u64));
        assert_eq!(product.to_be_bytes()[31], 6u8); // Check the least significant byte

        // Additional assertions to verify the values
        assert!(sum > two);
        assert!(sum > three);
        assert!(product > sum);
        assert_eq!(product, three.wrapping_add(&three)); // 2 * 3 = 3 + 3
    }
}
