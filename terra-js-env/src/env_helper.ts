let env = require("../env.json");
const fs = require("fs");

let env_name: string;
if(process.argv[2]){
	env_name = process.argv[2];
}else{
	env_name = "dev";
};
env = env[env_name];

function add_uploaded_token(codeName: string, address: string){

	let current_env = require("../env.json");
	if(!current_env[env_name]["cw20"]){
		current_env[env_name]["cw20"] = {};
	}
	current_env[env_name]["cw20"][codeName] = address;

	let data = JSON.stringify(current_env, undefined, 4);
	fs.writeFileSync('env.json', data);
}

function add_contract(contractName: string, address: string){
	let current_env = require("../env.json");
	if(!current_env[env_name]["contracts"]){
		current_env[env_name]["contracts"] = {};
	}
	current_env[env_name]["contracts"][contractName] = address;

	let data = JSON.stringify(current_env, undefined, 4);
	fs.writeFileSync('env.json', data);

}

export { env, add_uploaded_token, add_contract};