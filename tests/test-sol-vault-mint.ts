import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { HastraSolVaultMint } from "../target/types/hastra_sol_vault_mint";
import {
    createMint,
    createAccount,
    mintTo,
    getAccount,
    TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { assert } from "chai";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";

describe("sol-vault-mint", () => {
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);

    const program = anchor.workspace.HastraSolVaultMint as Program<HastraSolVaultMint>;

    // Test accounts
    let vaultMint: PublicKey; // USDC mint
    let mintToken: PublicKey; // wYLDS mint
    let vaultTokenAccount: PublicKey;
    let userVaultTokenAccount: PublicKey;
    let userMintTokenAccount: PublicKey;
    let configPda: PublicKey;
    let vaultAuthorityPda: PublicKey;
    let mintAuthorityPda: PublicKey;
    let freezeAuthorityPda: PublicKey;

    const user = provider.wallet;
    const freezeAdmin = Keypair.generate();
    const rewardsAdmin = Keypair.generate();

    before(async () => {
        // Airdrop SOL to test accounts
        await provider.connection.requestAirdrop(freezeAdmin.publicKey, 1000000000);
        await provider.connection.requestAirdrop(rewardsAdmin.publicKey, 1000000000);

        // Create vault mint (USDC)
        vaultMint = await createMint(
            provider.connection,
            user.payer,
            user.publicKey,
            null,
            6
        );

        // Create mint token (wYLDS)
        mintToken = await createMint(
            provider.connection,
            user.payer,
            user.publicKey,
            null,
            6
        );

        // Create token accounts
        vaultTokenAccount = await createAccount(
            provider.connection,
            user.payer,
            vaultMint,
            user.publicKey
        );

        userVaultTokenAccount = await createAccount(
            provider.connection,
            user.payer,
            vaultMint,
            user.publicKey
        );

        userMintTokenAccount = await createAccount(
            provider.connection,
            user.payer,
            mintToken,
            user.publicKey
        );

        // Mint some vault tokens (USDC) to user
        await mintTo(
            provider.connection,
            user.payer,
            vaultMint,
            userVaultTokenAccount,
            user.publicKey,
            1000000 // 1 token with 6 decimals
        );

        // Derive PDAs
        [configPda] = PublicKey.findProgramAddressSync(
            [Buffer.from("config")],
            program.programId
        );

        [vaultAuthorityPda] = PublicKey.findProgramAddressSync(
            [Buffer.from("vault_authority")],
            program.programId
        );

        [mintAuthorityPda] = PublicKey.findProgramAddressSync(
            [Buffer.from("mint_authority")],
            program.programId
        );

        [freezeAuthorityPda] = PublicKey.findProgramAddressSync(
            [Buffer.from("freeze_authority")],
            program.programId
        );
    });

    it("Initializes the program", async () => {
        const tx = await program.methods
            .initialize(
                vaultMint,
                mintToken,
                [freezeAdmin.publicKey],
                [rewardsAdmin.publicKey]
            )
            .accounts({
                vaultTokenAccount: vaultTokenAccount,
                vaultMint: vaultMint,
                mint: mintToken,
                signer: user.publicKey,
            })
            .rpc();

        // Verify config was created
        const config = await program.account.config.fetch(configPda);
        assert.equal(config.vault.toBase58(), vaultMint.toBase58());
        assert.equal(config.mint.toBase58(), mintToken.toBase58());
        assert.equal(config.freezeAdministrators.length, 1);
        assert.equal(config.rewardsAdministrators.length, 1);
    });

    it("Deposits vault tokens and mints wYLDS tokens", async () => {
        const depositAmount = new anchor.BN(100000); // 0.1 token

        const tx = await program.methods
            .deposit(depositAmount)
            .accounts({
                vaultTokenAccount: vaultTokenAccount,
                mint: mintToken,
                signer: user.publicKey,
                userVaultTokenAccount: userVaultTokenAccount,
                userMintTokenAccount: userMintTokenAccount,
            })
            .rpc();

        // Verify tokens were transferred and minted
        const vaultAccount = await getAccount(provider.connection, vaultTokenAccount);
        const userMintAccount = await getAccount(provider.connection, userMintTokenAccount);

        assert.equal(vaultAccount.amount.toString(), depositAmount.toString());
        assert.equal(userMintAccount.amount.toString(), depositAmount.toString());
    });

    it("Redeems wYLDS tokens for vault tokens", async () => {
        const redeemAmount = new anchor.BN(50000); // 0.05 token

        const tx = await program.methods
            .redeem(redeemAmount)
            .accounts({
                vaultTokenAccount: vaultTokenAccount,
                signer: user.publicKey,
                userVaultTokenAccount: userVaultTokenAccount,
                userMintTokenAccount: userMintTokenAccount,
                mint: mintToken,
            })
            .rpc();

        // Verify tokens were burned and transferred back
        const vaultAccount = await getAccount(provider.connection, vaultTokenAccount);
        const userMintAccount = await getAccount(provider.connection, userMintTokenAccount);

        assert.equal(vaultAccount.amount.toString(), "50000"); // 100000 - 50000
        assert.equal(userMintAccount.amount.toString(), "50000"); // 100000 - 50000
    });

    it("Creates rewards epoch", async () => {
        const epochIndex = new anchor.BN(1);
        const merkleRoot = Array(32).fill(0); // Mock merkle root
        const totalRewards = new anchor.BN(10000);

        const [epochPda] = PublicKey.findProgramAddressSync(
            [Buffer.from("epoch"), epochIndex.toArrayLike(Buffer, "le", 8)],
            program.programId
        );

        const tx = await program.methods
            .createRewardsEpoch(epochIndex, merkleRoot, totalRewards)
            .accounts({
                admin: rewardsAdmin.publicKey,
            })
            .signers([rewardsAdmin])
            .rpc();

        // Verify epoch was created
        const epoch = await program.account.rewardsEpoch.fetch(epochPda);
        assert.equal(epoch.index.toString(), epochIndex.toString());
        assert.equal(epoch.total.toString(), totalRewards.toString());
    });

    it("Fails unauthorized freeze attempt", async () => {
        const unauthorizedUser = Keypair.generate();

        try {
            await program.methods
                .freezeTokenAccount()
                .accounts({
                    tokenAccount: userMintTokenAccount,
                    mint: mintToken,
                    signer: unauthorizedUser.publicKey,
                })
                .signers([unauthorizedUser])
                .rpc();

            assert.fail("Should have failed due to unauthorized freeze administrator");
        } catch (error) {
            assert.include(error.toString(), "UnauthorizedFreezeAdministrator");
        }
    });

    it("Freezes user token account", async () => {
        const tx = await program.methods
            .freezeTokenAccount()
            .accounts({
                tokenAccount: userMintTokenAccount,
                mint: mintToken,
                signer: freezeAdmin.publicKey,
            })
            .signers([freezeAdmin])
            .rpc();

        // Verify account is frozen
        const account = await getAccount(provider.connection, userMintTokenAccount);
        assert.isTrue(account.isFrozen);
    });

    it("Thaws user token account", async () => {
        const tx = await program.methods
            .thawTokenAccount()
            .accounts({
                tokenAccount: userMintTokenAccount,
                mint: mintToken,
                signer: freezeAdmin.publicKey,
            })
            .signers([freezeAdmin])
            .rpc();

        // Verify account is thawed
        const account = await getAccount(provider.connection, userMintTokenAccount);
        assert.isFalse(account.isFrozen);
    });

    it("Updates freeze administrators", async () => {
        const newFreezeAdmin = Keypair.generate();
        await provider.connection.requestAirdrop(newFreezeAdmin.publicKey, 1000000000);

        const BPF_LOADER_UPGRADEABLE_ID = new PublicKey(
            "BPFLoaderUpgradeab1e11111111111111111111111"
        );
        const [programData] = PublicKey.findProgramAddressSync(
            [program.programId.toBuffer()],
            BPF_LOADER_UPGRADEABLE_ID
        );

        const tx = await program.methods
            .updateFreezeAdministrators([freezeAdmin.publicKey, newFreezeAdmin.publicKey])
            .accounts({
                programData: programData,
                signer: user.publicKey,
            })
            .rpc();

        // Verify administrators were updated
        const config = await program.account.config.fetch(configPda);
        assert.equal(config.freezeAdministrators.length, 2);
    });

    it("Prevents too many administrators", async () => {
        const tooManyAdmins = Array(6).fill(0).map(() => Keypair.generate().publicKey);

        const BPF_LOADER_UPGRADEABLE_ID = new PublicKey(
            "BPFLoaderUpgradeab1e11111111111111111111111"
        );
        const [programData] = PublicKey.findProgramAddressSync(
            [program.programId.toBuffer()],
            BPF_LOADER_UPGRADEABLE_ID
        );

        try {
            await program.methods
                .updateFreezeAdministrators(tooManyAdmins)
                .accounts({
                    programData: programData,
                    signer: user.publicKey,
                })
                .rpc();

            assert.fail("Should have failed due to too many administrators");
        } catch (error) {
            assert.include(error.toString(), "TooManyAdministrators");
        }
    });

    it("Claims rewards with valid merkle proof", async () => {
        const epochIndex = new anchor.BN(2);
        const claimAmount = new anchor.BN(5000);

        // Create epoch first
        const [epochPda] = PublicKey.findProgramAddressSync(
            [Buffer.from("epoch"), epochIndex.toArrayLike(Buffer, "le", 8)],
            program.programId
        );

        // Mock merkle proof - in real implementation, this would be computed off-chain
        const mockProof: number[][] = [];

        // For testing, create a simple merkle root that validates our claim
        const crypto = require('crypto');
        const userData = Buffer.concat([
            user.publicKey.toBuffer(),
            claimAmount.toArrayLike(Buffer, "le", 8),
            epochIndex.toArrayLike(Buffer, "le", 8)
        ]);
        const leafHash = crypto.createHash('sha256').update(userData).digest();

        await program.methods
            .createRewardsEpoch(epochIndex, Array.from(leafHash), claimAmount)
            .accounts({
                admin: rewardsAdmin.publicKey,
            })
            .signers([rewardsAdmin])
            .rpc();

        // Claim rewards
        const [claimRecordPda] = PublicKey.findProgramAddressSync(
            [Buffer.from("claim"), epochPda.toBuffer(), user.publicKey.toBuffer()],
            program.programId
        );

        const tx = await program.methods
            .claimRewards(claimAmount, mockProof)
            .accounts({
                user: user.publicKey,
                epoch: epochPda,
                mint: mintToken,
                userMintTokenAccount: userMintTokenAccount,
            })
            .rpc();

        // Verify claim record was created
        const claimRecord = await program.account.claimRecord.fetch(claimRecordPda);
        assert.isNotNull(claimRecord);
    });

    it("Prevents double claiming", async () => {
        const epochIndex = new anchor.BN(2);
        const claimAmount = new anchor.BN(5000);
        const mockProof: number[][] = [];

        const [epochPda] = PublicKey.findProgramAddressSync(
            [Buffer.from("epoch"), epochIndex.toArrayLike(Buffer, "le", 8)],
            program.programId
        );

        try {
            await program.methods
                .claimRewards(claimAmount, mockProof)
                .accounts({
                    user: user.publicKey,
                    epoch: epochPda,
                    mint: mintToken,
                    userMintTokenAccount: userMintTokenAccount,
                })
                .rpc();

            assert.fail("Should have failed due to double claim attempt");
        } catch (error) {
            // Account already exists error indicates claim record prevents double claiming
            assert.isTrue(error.toString().includes("already in use") ||
                error.toString().includes("RewardsAlreadyClaimed"));
        }
    });
});
