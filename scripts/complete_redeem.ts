import * as anchor from "@coral-xyz/anchor";
import yargs from "yargs";
import { Program } from "@coral-xyz/anchor";
import { HastraSolVaultMint } from "../target/types/hastra_sol_vault_mint";
import { PublicKey } from "@solana/web3.js";
import { getAssociatedTokenAddress } from "@solana/spl-token";

const provider = anchor.AnchorProvider.env();
anchor.setProvider(provider);

const program = anchor.workspace.HastraSolVaultMint as Program<HastraSolVaultMint>;

const args = yargs(process.argv.slice(2))
    .option("user", {
        type: "string",
        description: "The user's public key who made the redemption request.",
        required: true,
    })
    .option("mint", {
        type: "string",
        description: "The mint token that will be burned (e.g. wYLDS).",
        required: true,
    })
    .option("vaultMint", {
        type: "string",
        description: "The vault mint token (e.g. USDC) to transfer to user.",
        required: true,
    })
    .parseSync();

const main = async () => {
    const admin = provider.wallet.publicKey;
    const user = new PublicKey(args.user);
    const mint = new PublicKey(args.mint);
    const vaultMint = new PublicKey(args.vaultMint);

    // Derive PDAs
    const [configPda] = anchor.web3.PublicKey.findProgramAddressSync(
        [Buffer.from("config")],
        program.programId
    );

    const [redemptionRequestPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("redemption_request"), user.toBuffer()],
        program.programId
    );

    const [redeemVaultAuthorityPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("redeem_vault_authority")],
        program.programId
    );

    // Get token accounts
    const userMintTokenAccount = await getAssociatedTokenAddress(mint, user);
    const userVaultTokenAccount = await getAssociatedTokenAddress(vaultMint, user);
    const redeemVaultTokenAccount = await getAssociatedTokenAddress(vaultMint, redeemVaultAuthorityPda, true);

    console.log(`Admin: ${admin.toBase58()}`);
    console.log(`User: ${user.toBase58()}`);
    console.log(`Mint: ${mint.toBase58()}`);
    console.log(`Vault Mint: ${vaultMint.toBase58()}`);
    console.log(`Config PDA: ${configPda.toBase58()}`);
    console.log(`Redemption Request PDA: ${redemptionRequestPda.toBase58()}`);
    console.log(`Redeem Vault Authority PDA: ${redeemVaultAuthorityPda.toBase58()}`);

    const tx = await program.methods
        .completeRedeem() // Amount is calculated in the function
        .accountsStrict({
            admin: admin,
            user: user,
            userMintTokenAccount: userMintTokenAccount,
            userVaultTokenAccount: userVaultTokenAccount,
            redemptionRequest: redemptionRequestPda,
            redeemVaultTokenAccount: redeemVaultTokenAccount,
            redeemVaultAuthority: redeemVaultAuthorityPda,
            mint: mint,
            config: configPda,
            tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
        })
        .rpc();

    console.log("Complete redeem transaction:", tx);
};

main().catch(console.error);
