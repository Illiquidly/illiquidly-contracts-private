import { Address } from "../terra_utils";
import { env, add_uploaded_token, add_contract } from "../env_helper";

function getContractLog(response: any){
	return response.logs[0].eventsByType.from_contract;
}


/// Here we want to upload the p2p contract and add the fee contract
async function main(){

	// Getting a handler for the current address
	let handler = new Address(env["mnemonics"][0]);
	// Uploading the contract code	
	let loan = handler.getContract(env.contracts.loan);

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
	console.log(token_id);

	// We start the flow !!
	response = await nft.execute.approve({
		spender: loan.address,
		token_id: token_id,
	});
	console.log(response);

	response = await loan.execute.deposit_collateral({
		address: nft.address,
		token_id: token_id,
	})
	console.log(response);
	let loan_id = parseInt(getContractLog(response).loan_id[0]);

	response = await loan.execute.withdraw_collateral({
		loan_id: loan_id
	})
	console.log(response);

	// We verify the nft is still available
	response = await nft.query.tokens({owner: handler.getAddress()});
	console.log(response);

}

main().then(resp => {

}).catch(err => {
  console.log(err);
})
