/*
 Generated by typeshare 1.13.0
*/

/** Helper type for typeshare */
export type SecretStoreT = Record<string, string>;

export interface AddCertificateRequest {
	subject: string;
}

export enum TeamRole {
	Owner = "owner",
	Admin = "admin",
	Member = "member",
}

/**
 * Provide user id to add user.
 * Provide email address to invite user via email.
 */
export interface AddTeamMemberRequest {
	user_id?: string;
	email?: string;
	/** Role of the user in the team */
	role?: TeamRole;
}

export interface ApiError {
	message: string;
	status_code: number;
}

export interface BetterstackConfig {
	ingesting_host: string;
	source_token: string;
}

export interface BuildArgsRust {
	/** Version of shuttle-runtime used by this crate */
	shuttle_runtime_version?: string;
	/** Use the built in cargo chef setup for caching */
	cargo_chef: boolean;
	/** Build with the built in `cargo build` setup */
	cargo_build: boolean;
	/** The cargo package name to compile */
	package_name?: string;
	/** The cargo binary name to compile */
	binary_name?: string;
	/** comma-separated list of features to activate */
	features?: string;
	/** Passed on to `cargo build` */
	no_default_features: boolean;
	/** Use the mold linker */
	mold: boolean;
}

export interface BuildMeta {
	git_commit_id?: string;
	git_commit_msg?: string;
	git_branch?: string;
	git_dirty?: boolean;
}

export interface CertificateResponse {
	id: string;
	subject: string;
	serial_hex: string;
	not_after: string;
}

export interface CertificateListResponse {
	certificates: CertificateResponse[];
}

/** Holds the data for building a database connection string. */
export interface DatabaseInfo {
	engine: string;
	role_name: string;
	role_password: string;
	database_name: string;
	port: string;
	hostname: string;
	/**
	 * The RDS instance name, which is required for deleting provisioned RDS instances, it's
	 * optional because it isn't needed for shared PG deletion.
	 */
	instance_name?: string;
}

export interface DatadogConfig {
	api_key: string;
}

export interface DeleteCertificateRequest {
	subject: string;
}

export enum DeploymentState {
	Pending = "pending",
	Building = "building",
	Running = "running",
	InProgress = "inprogress",
	Stopped = "stopped",
	Stopping = "stopping",
	Failed = "failed",
	/** Fallback */
	Unknown = "unknown",
}

export interface DeploymentResponse {
	id: string;
	state: DeploymentState;
	created_at: string;
	updated_at: string;
	/** URIs where this deployment can currently be reached (only relevant for Running state) */
	uris: string[];
	build_id?: string;
	build_meta?: BuildMeta;
}

export interface DeploymentListResponse {
	deployments: DeploymentResponse[];
}

export type BuildArgs = 
	| { type: "Rust", content: BuildArgsRust }
	| { type: "Unknown", content?: undefined };

export interface DeploymentRequestBuildArchive {
	/** The S3 object version ID of the archive to use */
	archive_version_id: string;
	build_args?: BuildArgs;
	/**
	 * Secrets to add before this deployment.
	 * TODO: Remove this in favour of a separate secrets uploading action.
	 */
	secrets?: Record<string, string>;
	build_meta?: BuildMeta;
}

export interface DeploymentRequestImage {
	image: string;
	/** TODO: Remove this in favour of a separate secrets uploading action. */
	secrets?: Record<string, string>;
}

export interface GrafanaCloudConfig {
	token: string;
	endpoint: string;
	instance_id: string;
}

export interface LogItem {
	timestamp: string;
	/** Which container / log stream this line came from */
	source: string;
	line: string;
}

export interface LogsResponse {
	logs: LogItem[];
}

export interface ProjectCreateRequest {
	name: string;
}

export enum ComputeTier {
	XS = "xs",
	S = "s",
	M = "m",
	L = "l",
	XL = "xl",
	XXL = "xxl",
}

export interface ProjectResponse {
	id: string;
	/** Display name */
	name: string;
	/** Project owner */
	user_id: string;
	/** Team project belongs to */
	team_id?: string;
	created_at: string;
	compute_tier?: ComputeTier;
	/** State of the current deployment if one exists (something has been deployed). */
	deployment_state?: DeploymentState;
	/** URIs where running deployments can be reached */
	uris: string[];
}

export interface ProjectListResponse {
	projects: ProjectResponse[];
}

/** Set wanted field(s) to Some to update those parts of the project */
export interface ProjectUpdateRequest {
	/** Change display name */
	name?: string;
	/** Transfer to other user */
	user_id?: string;
	/** Transfer to a team */
	team_id?: string;
	/** Transfer away from current team */
	remove_from_team?: boolean;
	/** Change compute tier */
	compute_tier?: ComputeTier;
}

export enum ResourceType {
	DatabaseSharedPostgres = "database::shared::postgres",
	DatabaseAwsRdsPostgres = "database::aws_rds::postgres",
	DatabaseAwsRdsMySql = "database::aws_rds::mysql",
	DatabaseAwsRdsMariaDB = "database::aws_rds::mariadb",
	/** (Will probably be removed) */
	Secrets = "secrets",
	/** Local provisioner only */
	Container = "container",
}

export interface ProvisionResourceRequest {
	/** The type of this resource */
	type: ResourceType;
	/**
	 * The config used when creating this resource.
	 * Use `Self::r#type` to know how to parse this data.
	 */
	config: any;
}

/** The resource state represents the stage of the provisioning process the resource is in. */
export enum ResourceState {
	Authorizing = "authorizing",
	Provisioning = "provisioning",
	Failed = "failed",
	Ready = "ready",
	Deleting = "deleting",
	Deleted = "deleted",
}

export interface ResourceResponse {
	type: ResourceType;
	state: ResourceState;
	/** The config used when creating this resource. Use the `r#type` to know how to parse this data. */
	config: any;
	/** The output type for this resource, if state is Ready. Use the `r#type` to know how to parse this data. */
	output: any;
}

export interface ResourceListResponse {
	resources: ResourceResponse[];
}

export enum SubscriptionType {
	Pro = "pro",
	Rds = "rds",
}

export interface Subscription {
	id: string;
	type: SubscriptionType;
	quantity: number;
	created_at: string;
	updated_at: string;
}

export interface SubscriptionRequest {
	id: string;
	type: SubscriptionType;
	quantity: number;
}

export interface TeamMembership {
	user_id: string;
	/** Role of the user in the team */
	role: TeamRole;
}

export interface TeamResponse {
	id: string;
	/** Display name */
	name: string;
	/** Membership info of the calling user */
	membership: TeamMembership;
}

export interface TeamListResponse {
	teams: TeamResponse[];
}

export interface TeamMembersResponse {
	members: TeamMembership[];
}

/** Status of a telemetry export configuration for an external sink */
export interface TelemetrySinkStatus {
	/** Indicates that the associated project is configured to export telemetry data to this sink */
	enabled: boolean;
}

/** A safe-for-display representation of the current telemetry export configuration for a given project */
export interface TelemetryConfigResponse {
	betterstack?: TelemetrySinkStatus;
	datadog?: TelemetrySinkStatus;
	grafana_cloud?: TelemetrySinkStatus;
}

export interface UploadArchiveResponse {
	/** The S3 object version ID of the uploaded object */
	archive_version_id: string;
}

export enum AccountTier {
	Basic = "basic",
	/** A basic user that is pending a payment on the backend */
	PendingPaymentPro = "pendingpaymentpro",
	CancelledPro = "cancelledpro",
	Pro = "pro",
	Growth = "growth",
	/** Higher limits and partial admin endpoint access */
	Employee = "employee",
	/** Unlimited resources, full API access, admin endpoint access */
	Admin = "admin",
}

export interface UserResponse {
	id: string;
	/** Auth0 id */
	name: string;
	key: string;
	account_tier: AccountTier;
	subscriptions: Subscription[];
	flags?: string[];
}

export type DeploymentRequest = 
	/** Build an image from the source code in an attached zip archive */
	| { type: "BuildArchive", content: DeploymentRequestBuildArchive }
	/** Use this image directly. Can be used to skip the build step. */
	| { type: "Image", content: DeploymentRequestImage };

/** The user-supplied config required to export telemetry to a given external sink */
export type TelemetrySinkConfig = 
	/** [Betterstack](https://betterstack.com/docs/logs/open-telemetry/) */
	| { type: "betterstack", content: BetterstackConfig }
	/** [Datadog](https://docs.datadoghq.com/opentelemetry/collector_exporter/otel_collector_datadog_exporter) */
	| { type: "datadog", content: DatadogConfig }
	/** [Grafana Cloud](https://grafana.com/docs/grafana-cloud/send-data/otlp/) */
	| { type: "grafana_cloud", content: GrafanaCloudConfig };

