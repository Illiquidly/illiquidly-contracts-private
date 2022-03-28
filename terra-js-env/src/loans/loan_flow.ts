import { Address } from "../terra_utils";
import { env, add_uploaded_token, add_contract } from "../env_helper";
import { Numeric } from "@terra-money/terra.js";
function getContractLog(response: any){
	return response.logs[0].eventsByType.from_contract;
}


/// Here we want to upload the p2p contract and add the fee contract
async function main(){

	// Getting a handler for the current address
	let handler = new Address(env["mnemonics"][0]);
	let anyone = new Address(env["mnemonics"][1]);
	// Uploading the contract code	
	let loan = handler.getContract(env.contracts.loan);
	let loan_anyone = anyone.getContract(env.contracts.loan);

	let cw721_tokens = env["cw721"];
	let cw721_token_names = Object.keys(cw721_tokens);
	let nft = handler.getContract(cw721_tokens[cw721_token_names[0]]);
	let response = await nft.query.tokens({owner: handler.getAddress()});
	let token_id;
	if(response.tokens.length == 0){
		console.log("Mint new token");
		token_id = handler.getAddress() + Math.ceil(Math.random()*10000)
		await nft.execute.mint({
			token_id: token_id,
			owner: handler.getAddress(),
			token_uri: "testing"
		})
	}else{
		token_id = response.tokens[0];
	}
	console.log("Token id tested :",token_id);

	// We start the flow !!
	response = await nft.execute.approve({
		spender: loan.address,
		token_id: token_id,
	});
	console.log("Approved nft");

	response = await loan.execute.deposit_collateral({
		address: nft.address,
		token_id: token_id,
	})
	console.log("Deposited Collateral");
	let loan_id = parseInt(getContractLog(response).loan_id[0]);

	// Shouldn't be possible, there are no terms
	await loan_anyone.execute.accept_loan({
		borrower : handler.getAddress(),
    loan_id: loan_id
	})

	response = await loan_anyone.execute.make_offer({
		borrower : handler.getAddress(),
    loan_id: loan_id,
    terms: {
    	principle: {
	    	amount:"500",
	    	denom:"uluna"
	    },
	    interest: "50",
	    duration_in_blocks: 50
	  }
	}, "500uluna")
	let offer_id = parseInt(getContractLog(response).offer_id[0]);
	console.log("Offer made");

	let balance_before: Numeric.Output = (await handler.terra.bank.balance(handler.getAddress()))[0].get("uluna")!.amount;

	await loan.execute.accept_offer({
		loan_id: loan_id,
		offer_id: offer_id
	})
	console.log("Offer accepted");

	let balance_after: Numeric.Output = (await handler.terra.bank.balance(handler.getAddress()))[0].get("uluna")!.amount;
	console.log("Balance difference", balance_after.sub(balance_before));

	// Not possible now
	await loan.execute.withdraw_collateral({
		loan_id: loan_id
	})
	await loan.execute.accept_offer({
		loan_id: loan_id,
		offer_id: offer_id
	})

	// Now we want to repay the loan please ?
	// Not enough funds sent my friend
	await loan.execute.repay_borrowed_funds({
		loan_id: loan_id,
	}, "520uluna");

	// Finally my precious NFT
	await loan.execute.repay_borrowed_funds({
		loan_id: loan_id,
	}, "550uluna");
	console.log("Loan is ended, this is over, I can move on and derisk my position");

	// We verify the nft is still available
	response = await nft.query.tokens({owner: handler.getAddress()});
	console.log(response);
}

main().then(resp => {

}).catch(err => {
  console.log(err);
})
